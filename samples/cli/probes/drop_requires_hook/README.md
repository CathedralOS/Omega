# drop_requires_hook

`machine Token::drop(token: &mut Token) requires token.value == 42` — a
drop hook carrying a contract: the cleanup edge only attaches where the
requires is judged. A partially-constructed `Pair` builds only
`pair.right`; the untouched `left` storage receives neither replacement
nor exit cleanup.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
