# Gamma language

Gamma is the safe definitional language in the audited bootstrap spine. Its
canonical meaning is the pure, fuel-bounded reference interpreter in
`interp.beta`; its static checker is `typeck.beta`. Both are Beta programs built
by the self-hosting Beta compiler.

The old imperative compiler-first prototype is retired to Git history. It is
not the canonical Gamma surface.

## Evaluated surface

The reference interpreter accepts prefix expressions:

```text
program := (def NAME (PARAM...) EXPR)* EXPR
EXPR    := INT | VAR
         | (if EXPR EXPR EXPR)
         | (let NAME EXPR EXPR)
         | (+ EXPR EXPR) | (- EXPR EXPR) | (* EXPR EXPR)
         | (/ EXPR EXPR) | (% EXPR EXPR)
         | (eq EXPR EXPR) | (lt EXPR EXPR)
         | (NAME EXPR...)
         | CONSTRUCTOR | (CONSTRUCTOR EXPR...)
         | (match EXPR (PATTERN EXPR)...)
PATTERN := NAME | CONSTRUCTOR | (CONSTRUCTOR NAME...)
```

Uppercase heads are algebraic-data constructors. A lowercase pattern name is a
catch-all binding. Evaluation is pure; recursion is bounded by explicit
interpreter fuel, so exhaustion is a reference-evaluation outcome rather than an
invisible unbounded computation. Tail-position `let`, `if`, `match`, and call
chains are trampolined: a terminating tail-recursive Gamma program does not also
depend on the Beta/Alpha return-stack depth. The interpreter may represent a
bounded integer range immediately and compact ordinary two-field `Cons` cells
internally. It may likewise
compact the ordinary `Node` and `Chunks` constructors used by the bootstrap
translator's persistent-array carrier; matching and the canonical printed
constructor tree are unchanged by those representations. It may classify known
constructor patterns while parsing as long as arbitrary constructor names and
exact constructor arities retain the same matching behavior. The canonical
interpreter may also use bounded private scratch storage to transfer
already-evaluated call arguments; those slots are never Gamma values and are
released when the callee parameters are bound. It may cache the function-table
index resolved for a parsed call expression; the cache is private evaluator
metadata and leaves source name resolution, argument evaluation, fuel, and
observable values unchanged. Exhausting the canonical
interpreter's private source, argument, or node capacities is a fail-closed host
outcome and never publishes a partial Gamma value. Parsed syntax is pinned for
the evaluation. A parsed variable expression may likewise cache its resolved
slot relative to the current function frame: Gamma has no closures, lookup does
not cross that frame, and the slot is fixed by the expression's lexical
position, so repeated calls and recursive re-entry still read the current
binding. Runtime values may be reclaimed by a stable-address,
representation-aware conservative collector: candidate roots must decode to
exact live allocation starts, so conservative retention cannot change values,
matching, evaluation order, or printed constructor trees. Exhausting the
runtime heap after reclamation remains the same fail-closed host outcome.
The evaluator checks fuel at its external entry and before each decremented
function-body transfer. Internal subexpression evaluation preserves the
positive-fuel invariant, so eliminating duplicate child-level checks does not
change the fuel-bounded meaning.

## Statically checked surface

Typed Gamma adds explicit algebraic-data and function declarations:

```text
program := (data TYPE (CONSTRUCTOR TYPE...)...)*
           (def NAME ((PARAM TYPE)...) RETURN_TYPE EXPR)*
```

`Int` is built in. The type system is monomorphic and fully annotated. It checks
function and constructor arity, operator operands, calls, return types, match
scrutinees, pattern constructors, and agreement between match arms. It is
deliberately small: enough to make interpreters, validators, decoders, and the
independent Gamma proof-kernel implementation safe to write.

## Gates and examples

```sh
sh source/gamma/test-interp.sh
sh source/gamma/test-typeck.sh
sh source/alpha/checker/gates/gamma-checker.sh
```

The typed Gamma proof-kernel implementation lives under the checker that owns it
at `source/alpha/checker/implementations/gamma/checker_typed.gamma`. The old
generic canonical-byte and terminal-codec prototype was not consumed by a live
artifact admission and is retired to Git history; future artifact-specific
decoding belongs beside the artifact it admits.
