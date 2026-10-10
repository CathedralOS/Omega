# compound_application_binder

Application binders inside a compound satisfies argument: `machine Reader::read<Element, const Width: u32, Value>(field: &[Element; Width], value) satisfies Readable<[Element; Width]>::read` — impl telescope genericizes through the instantiated `[Element; Width]`; call sites take explicit static args `Reader::read<u8, 2, i32>`/`Reader::read<u16, 3, u64>`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
