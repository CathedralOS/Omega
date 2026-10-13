# Sibling-module service exclusion

`omega run tests/omega/pass/build/sibling_module_service_exclusion_exit/main.omg --both`
after ordinary package review returns zero in both engines.

Modules `left` and `right` each declare a boundary trait `Sink`. A build
helper excludes `left::Sink`; the product invokes `right::Sink` through a
helper machine. The exclusion selects the declaration by its package and
module identity, so the sibling with the same name stays admitted.
`fail/build/static_service_exclusion_relayed` invokes `left::Sink` instead
and must reject.
