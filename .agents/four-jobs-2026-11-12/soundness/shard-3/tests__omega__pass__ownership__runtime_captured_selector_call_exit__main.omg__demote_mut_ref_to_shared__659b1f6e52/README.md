Run `omega run --both tests/omega/pass/ownership/runtime_captured_selector_call_exit/main.omg`.

The authored exit is 0. The selector is an effectful scalar computation whose
contract proves its result is one. It must run once before the structural RHS
call. Old-value cleanup and store use that completed operand, while the caller
observes the selector call count and the RHS's later cursor update.
The preceding live element also detects cleanup that drops the runtime offset.
