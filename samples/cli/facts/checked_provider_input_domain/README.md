# Checked provider input domains

The caller may supply any `Index`. The provider tests `Index::Bounded` before passing the value to its lookup machine. An index outside that domain returns zero. For a two-element array storing 7 and 11, the checked index 1 returns 11.

The negative programs widen the domain from `value <= 1` to `value <= 2`. The provider can then reach the lookup with index 2, so the checker refuses the array access. The examples exercise values passed through locals, fields, parameters, call results, joins, constants, nested machines, other machines and target-selected providers.

[Provider contract migration](../../../../wiki/spec/resources/native_borrows.md#provider-contracts-and-migration)
