# nested_operand_view

Representation recasts compose as nested operands inside an ordinary
binary expression, in both shared (`as &`) and mutable (`as &mut`)
spellings: a mutable view read through an expression position
re-reads the same backing place — no reference-typed `let` binding
required.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
