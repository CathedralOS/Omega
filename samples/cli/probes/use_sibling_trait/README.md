# use_sibling_trait

`use contracts::drawable` brings a sibling file's trait into scope;
`Sprite::draw` satisfies it with the unqualified trait name — a file in
a subdirectory needs no module declaration to be imported.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
