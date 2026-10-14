# Explicit static Binding authority

`omega run main.omg` must exit 0. The program sends 65 through a selected,
checked local provider, stores it through an ordinary argument, and checks the
returned value. The computed authority sets a separate phase counter to 1;
its ordinary argument must observe 1 before advancing it to 2. A wrong result
reaches `crash Trap`.

The sample exercises statement, expression, projected, forwarded-reference and
computed authorities, nested boundary arguments, and explicit result discard.
Its local benign provider keeps this compiler probe independent of unrelated
foreign package admission.
