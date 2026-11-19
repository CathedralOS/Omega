# Static service exclusion

`omega run tests/omega/pass/build/runtime_static_service_exclusion_exit/main.omg --both`
after ordinary package review returns zero in both engines.

An authorized build helper forwards the original mutable Build argument to
`Build::exclude_service`. An ordinary generic machine with the same spelling
still resolves normally. The product calls a quiet helper whose published
contract allows Sink but whose body never invokes it.
