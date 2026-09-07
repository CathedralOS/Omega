# Bootstrap chain alternatives

> **Status: concatenative Gamma selected after implementation measurement.** The live
> direction is `Alpha -> Beta -> Gamma -> Delta -> Epsilon -> Omega`. Alpha
> remains the portable tape machine. Beta is the trusted imperative tape-
> assembly language; Gamma is the bounded concatenative compiler machine; Delta and Epsilon
> are the typed functional and fixed-storage compiler languages.

## Question

What is the smallest bootstrap root and language progression from which a
reviewer can understand, reconstruct, and check the first full Omega compiler?

The old formulation asked how to minimize every rung in an already selected
chain. That is too narrow. A small rung still has negative value when deleting
it makes the complete trust argument smaller. Every candidate is therefore
measured as one system:

```text
native execution root
  + directly admitted seed artifacts
  + language and machine semantics
  + compilers, interpreters, and checkers
  + exact artifact relations and certificates
  + resource and failure contracts
  + permanent construction and validation machinery
```

Development convenience, current corpus use, prior investment, and hypothetical
future reuse do not justify a rung. Generality receives no credit unless a named
required program uses it and the reuse reduces the complete audited system.

## A legitimate root need not have lower-language pedigree

An exact Alpha tape may be admitted as part of the audited root. It does not
need source in another bootstrap language merely to acquire a history.

The claim is then deliberately modest: these exact bytes, decoded under the
written Alpha instruction set, implement the stated small machine or evaluator.
Review uses the byte listing, control-flow reconstruction, semantic tests, and
any available proof. A host assembler or disassembler may help construct and
inspect the tape without becoming an authoritative compiler stage.

This separates two properties that the earlier ladder conflated:

- **pedigree:** which earlier program produced an artifact; and
- **auditability:** how cheaply the artifact's behavior can be understood and
  checked against its specification.

Pedigree is useful for reproducibility. It does not automatically make a longer
chain easier to trust.

## Candidate families

Names below identify experiments, not promised permanent products.

### A. Existing general-purpose ladder

```text
Alpha VM
  -> Beta textual assembler
  -> Gamma imperative/state-machine compiler language
  -> Delta pure functional language
  -> Epsilon typed compiler-host language
  -> Omega
```

This is the comparison baseline because most of it already exists. Its possible
advantage is that each compiler is written in a more expressive successor of a
smaller predecessor. Its cost is one specification, compiler, artifact edge,
resource model, and validation surface per rung. It wins only if those
intermediate steps make the total system smaller than the skip-rung candidates.

### B. Assembly directly to a functional rung

```text
Alpha VM
  -> Beta textual assembler
  -> small functional language
  -> Epsilon
  -> Omega
```

This candidate deletes Gamma. It tests whether the simple S-expression parser,
pure values, pattern matching, and structural evaluation needed by a functional
compiler are cheaper to implement directly in Beta than Gamma plus its compiler
and proof edge. Beta remains only if readable authoritative assembly is worth
more than directly auditing the resulting seed tape.

### C. Directly audited functional evaluator seed (rejected after prototype)

```text
Alpha VM + one exact audited functional-evaluator tape
  +-- executes functional proof checker
  `-- executes Gamma-written Delta compiler
        -> Delta-written Epsilon compiler
        -> Epsilon-written Omega compiler
        -> Omega self-host
```

This was the initially selected architecture. The evaluator is part of the root
rather than the output of a permanent assembly-language rung. Its language should be
only large enough for the checker and Delta compiler: closed bytes and
integers, immutable bindings, constructors, calls, conditionals, pattern
matching, structural recursion or tail calls, bounded allocation, sealed byte
input/output, and explicit failure.

The functional calculus is not a general-purpose Lisp. It has
no closures, higher-order values, macros, polymorphism, general garbage
collector, continuations, exceptions, modules, packages, mutation, raw memory,
interactive evaluator, or ambient effects. Any proposed addition must reduce
the complete audited chain for a named Gamma-compiler or checker workload.

The 42-case evaluator development slice assembled to 12,716 bytes before it
implemented declarations, general calls, constructors, `match`, or proper tail
calls. That measurement falsified the premise that the completed evaluator
would be a credible standalone instruction-audit subject. Calling its readable
source optional would merely hide the practical trust dependency.

### D. Beta-authored functional evaluator (selected)

```text
Alpha VM
  -> admitted Beta compiler tape and exact Beta self-reconstruction
  -> Beta-authored Gamma evaluator tape
  -> Gamma-authored Delta compiler
  -> Delta-authored Epsilon compiler
  -> Epsilon-authored Omega compiler
  -> Omega self-host
```

This is the selected architecture. It has the same functional semantics as
candidate C, but retains Beta to make the evaluator source readable and exactly
reconstructible. Beta's compiler is itself Beta source with an admitted
1,773-byte Alpha tape and byte-identical reconstruction. That added language
edge is more honest and reviewable than treating the larger Gamma evaluator
tape as independently understandable opaque root material.

### E. Directly audited functional compiler seed

```text
Alpha VM + one exact audited functional-compiler tape
  -> compiled functional checker and Delta compiler
  -> Delta -> Epsilon -> Omega
```

This replaces interpretation with compilation. It may improve bootstrap time
and make generated Alpha tapes easier to bind, but the root program is likely
larger because it owns lowering, layout, and code emission. It earns selection
only if those costs are smaller than retaining an evaluator plus its execution
argument. Performance alone does not enlarge the root unless the evaluator is
operationally unusable.

### F. Functional language directly to Omega

```text
Alpha VM + audited functional evaluator or compiler
  -> functional Omega compiler
  -> Omega self-host
```

This asks whether the fixed-storage compiler-host rung is necessary. It has the fewest semantic layers, but
may make the first Omega compiler, its resource behavior, or its proof much
larger. Epsilon survives only if a measured functional-to-Omega prototype is
harder to audit than the Epsilon language, Epsilon compiler, Epsilon-written
Omega compiler, and both associated edges together.

### G. Direct state-machine seed

```text
Alpha VM + audited state-machine evaluator/compiler tape
  -> state-machine Omega compiler
  -> Omega self-host
```

This is the control against the assumption that a functional rung is uniquely
cheap. A state-machine language may map more directly to Alpha and bound memory
more visibly, while requiring substantially more authored control flow. It
should compile the same representative workloads as the functional candidates
before either style is credited with simplicity.

### H. Multiple native language seeds

```text
native per-platform functional or state-machine seed
  -> higher compilers
```

This removes Alpha from the execution path but multiplies the native trusted
surface across hosts. It is retained as a negative control and may win only if
Alpha plus its seed artifact is demonstrably harder to audit than every native
implementation and their behavioral-equivalence argument. Convenience or
speed is insufficient.

## Independent design axes

The candidate list is not an invitation to implement every permutation. It
exposes five decisions that can be varied independently when a comparison
identifies the source of cost:

| Axis | Choices worth measuring |
| --- | --- |
| Root artifact | native seed, raw Alpha tape, or assembler-derived tape |
| First language mechanism | evaluator, bytecode compiler, or direct compiler |
| First source style | functional, state-machine, or assembly |
| Checker placement | root kernel, program in the first source language, or later checked extension |
| Upper bridge | first language directly to Omega, or through Epsilon |

Terminal Psi is not automatically a bootstrap rung. It is a portable product
compiler boundary and enters this comparison only if a concrete candidate uses
it to reduce the first Omega compiler and includes the required lowerer and
authority contract in its complete cost.

## Common experiment contract

Implementing alternatives is cheap relative to permanently reviewing them.
Prototypes may therefore be broad, but comparisons must share subjects and
measurements.

### Fixed subjects

Each surviving candidate must eventually perform the same work:

1. execute a small positive and negative language conformance suite;
2. run the same proof-checking kernel over the same representative valid and
   invalid derivations;
3. consume one frozen representative compiler source and emit exact Alpha tape;
4. consume the selected complete first-Omega-compiler source closure; and
5. produce an Omega compiler that accepts the selected self-host source.

Early prototypes may use smaller closures, but a result over a toy compiler is
not extrapolated to the complete edge. The target workload, accepted subset,
resource profile, and admissions accompany every result.

### Audit measurements

For every candidate publish at least:

- native trusted source and executable bytes per platform;
- directly admitted tape bytes and decoded instruction count;
- semantic rules, grammar productions, primitive operations, and failure
  categories for every retained language;
- compiler, evaluator, and checker source and artifact sizes;
- mutable tables, memory bounds, stack/fuel rules, and worst measured use;
- number and size of exact artifact relations, proof rules, certificates, and
  sidecar formats;
- clean reconstruction time and peak memory;
- permanent tests, tools, and host glue required after selection; and
- reviewer effort: time to explain the complete root, reconstruct control flow,
  and find seeded semantic defects.

Line count and tape bytes are evidence, not the objective. A smaller binary
that depends on clever undocumented invariants loses to a slightly larger one
whose behavior a reviewer can enumerate.

### Adversarial audit trial

Each serious candidate receives the same bounded mutation exercise. Seed bugs
in instruction decoding, arithmetic bounds, allocation, control transfer,
parser acceptance, artifact emission, and proof admission. Independent
reviewers receive only the candidate's declared audit package. Record which
defects are found, how long localization takes, and which claims cannot be
checked locally.

This is the only direct measurement of the property the chain is optimizing.
Compilation speed and implementation effort are secondary.

## Checker authority

Moving the checker into the first readable functional language improves its
reviewability but does not, by itself, prove the evaluator or compiler that
produced it. Each candidate must state the non-circular root claim:

- a directly audited evaluator may execute a readable checker whose semantics
  are included in the root audit;
- a smaller root kernel may validate a richer checker, but the kernel's full
  semantics and encoding count against the candidate; or
- the seed and checker may be explicitly admitted together and audited as one
  root subject.

No candidate receives a free proof edge merely because its checker is written
in a pleasant language.

## Selection and repository policy

Candidate C's evaluator-root shape is selected, retaining the renamed Gamma and
Delta upper languages. Losing permanent semantics, artifacts, gates, sidecars,
and compatibility paths are deleted; Git retains their history. A future
topology change requires a new whole-chain comparison rather than parallel live
authority.

LLM-generated code lowers prototype cost; it does not lower the human cost of
reviewing and maintaining another accepted trust path. “Build them all” is an
experiment strategy, not a retention strategy.

## Decision rule

A candidate wins when independent reviewers can understand and challenge its
complete trusted path more cheaply than every viable alternative while it
still constructs the required Omega compiler within explicit resource bounds.

No individual rung is presumed necessary. Beta, Gamma, Delta, Epsilon, the
checker architecture, and even the use of an interpreter rather than a compiler
are hypotheses to test. Alpha remains the current common execution baseline,
not an exemption from measurement.
