# sibling trait impl

Compile-scope probe. No standard-library dependency.

- `use contracts::drawable;` imports `trait Drawable<Sprite,
  Canvas>` from a sibling directory file (no `module` decl needed);
  `machine Sprite::draw ... satisfies Drawable<Sprite,
  Canvas>::draw` binds it.
- `let mut canvas: Canvas;` / `let sprite: Sprite;` zero-init
  declarations accepted.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/sibling_trait_impl/main.omg
mbx run -p omega -- update --project samples/cli/probes/sibling_trait_impl
```
