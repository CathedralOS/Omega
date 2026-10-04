# Build helper contracts remain checked

An executed Build helper returns 3 while promising 4. Its contract must reject
even though unrelated Product declarations have their own domain facts.

Run `omega --check tests/omega/fail/build/helper_contract_context/main.omg`.
The rejection must name the false helper proof, not a missing Product domain.
