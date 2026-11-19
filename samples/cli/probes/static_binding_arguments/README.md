# Explicit static Binding authority

`omega run main.omg` must exit 0. The program sends 65 through a selected,
checked local provider, stores it through an ordinary argument, and checks the
returned value. The computed authority sets a separate phase counter to 1;
its ordinary argument must observe 1 before advancing it to 2. A wrong result
reaches `crash Trap`.

The sample exercises statement, expression, forwarded-reference and computed
authority, nested boundary arguments, and explicit result discard. Its local
benign provider keeps this compiler probe independent of unrelated foreign
package admission.

## Migration

Until 2026-10-20 every call passed a `Binding<Character>` receiver —
`&context.character` at the call sites, `&Binding<Character>` parameters in
`write` and `authority`, and a `Binding<Character>` field in `Main`. The
specification retired source-visible `Binding<R>`
(`wiki/spec/build/component_publication.md:198`), so the carrier is gone and the
calls are requirement applications with ordinary arguments:
`Character::store(output, letter())` and
`Character::echo(ordinary_argument(&mut context.phase))`. The authority lane now
returns the phase borrow it took (`&'loan mut i32`) rather than forwarding a
carrier reference, and the projected receiver is the plain `context.value` this
program actually operated on.
