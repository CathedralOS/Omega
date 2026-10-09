# Declared carry floors compose through stored fields

Run `omega --check tests/omega/fail/data/property_carry_nested_restrictive_floor/main.omg`.
The outer declaration cannot promise portable carry while storing a value with
a stricter authored floor. The inner declaration's scalar field is portable;
its declared policy still contributes all four restrictions.
