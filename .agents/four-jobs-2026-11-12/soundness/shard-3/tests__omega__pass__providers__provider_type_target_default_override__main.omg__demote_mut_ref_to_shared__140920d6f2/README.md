The target package supplies FirstProvider as its default. Build selects SecondProvider instead, and main traps unless the selected implementation returns 2.

Provider-default selection is a compiler operation, so checking it must not require an ordinary function body to describe stored-reference changes. The exact retained operation supplies that fact; its arguments still execute before selection. This is a Cathedral prerequisite, not an installation or hardware test.

The ordinary same-name controls are [the executable reference replacement](../../borrows/ordinary_select_provider_reference_exit/main.omg) and [the rejected live-source mutation](../../../fail/borrows/ordinary_select_provider_reference_live_write/main.omg). They retain ordinary call effects even though the function is named select_provider.
