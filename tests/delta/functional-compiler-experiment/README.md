# Scalar Functional Delta compiler experiment gate

`run.sh` compiles the test-owned `compiler.gamma` scalar Functional Delta
compiler with both
the canonical Gamma evaluator and Gamma's native fixed-point compiler. It then
requires byte-identical compilation of a recursive customer, executes the
result under Alpha, and checks focused scalar and malformed programs.

The gate is evidence for one incremental implementation milestone only. It
does not stand in for Delta conformance or produce the canonical
`delta_compiler_bytecode.tape`.