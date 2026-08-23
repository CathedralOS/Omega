# Gamma language

Gamma is the safe definitional language in the audited bootstrap spine. Its
canonical meaning is the pure, fuel-bounded reference interpreter in
`interp.beta`; its static checker is `typeck.beta`. Both are Beta programs built
by the self-hosting Beta compiler.

The old imperative language compiled by `gamma.alpha` is a parked compatibility
artifact. It is not the canonical Gamma surface. Its historical programs remain
under `examples/` and are built only by the legacy `build.sh` path.

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
depend on the Beta/Alpha return-stack depth. The interpreter may intern bounded
integers and compact ordinary two-field `Cons` cells internally. It may likewise
compact the ordinary `Node` and `Chunks` constructors used by the bootstrap
translator's persistent-array carrier; matching and the canonical printed
constructor tree are unchanged by those representations. The canonical
interpreter may also use bounded private scratch storage to transfer
already-evaluated call arguments; those slots are never Gamma values and are
released when the callee parameters are bound.

## Statically checked surface

Typed Gamma adds explicit algebraic-data and function declarations:

```text
program := (data TYPE (CONSTRUCTOR TYPE...)...)*
           (def NAME ((PARAM TYPE)...) RETURN_TYPE EXPR)*
```

`Int` is built in. The type system is monomorphic and fully annotated. It checks
function and constructor arity, operator operands, calls, return types, match
scrutinees, pattern constructors, and agreement between match arms. It is
deliberately small: enough to make interpreters, validators, canonical-byte
decoders, and the independent Gamma proof-kernel implementation safe to write.

## Gates and examples

```sh
sh bootstrap/rungs/gamma/test-interp.sh
sh bootstrap/rungs/gamma/test-typeck.sh
sh bootstrap/assurance/proof-kernel/gates/gamma-checker.sh
```

Typed Gamma consumers live in `canonical-bytes/`, the reusable grammar fragments
under `terminal-codec-primitives/`, and
`bootstrap/assurance/proof-kernel/implementations/gamma/checker_typed.gamma`.
They exercise the language but do not define it. The retired terminal-ledger
prototype was artifact-assurance feasibility work, not Gamma meaning. The root
`examples/*.gamma` corpus belongs to the parked imperative compiler and must not
be used as the canonical language definition.
