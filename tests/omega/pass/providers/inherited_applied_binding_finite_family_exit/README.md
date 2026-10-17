# Applied ancestor binding with a finite requirement family

Run `omega run --both main.omg`. Expected exit: 0 in both engines.

One selected Child map supplies the exact Parent<i32> requirement application. Its outer
argument participates in the parameter and result, while two complete local
const tuples select ordinary specialized bodies. The uncalled spare requirement
remains in the whole-map roster; no separate Parent plan is selected.
