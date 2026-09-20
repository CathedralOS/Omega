//! Offset-bound meet coverage for strided dependent views.
//!
//! The dependent-values spec fixes the view contract: a view's runtime
//! witnesses (`count`, `stride`, `map_size`) are ordinary values bound at
//! establishment, and a live dependent view reads them as loans. The
//! byte-region recast's offset meet bounds `offset + size(T)` from the
//! per-edge incoming guards only while each witness reaches the next entry
//! UNCHANGED. These tests pin both sides of that fence on the Cathedral
//! stride walk: witnesses self-forwarded prove `off' + sizeof(Desc) <= 64`
//! through `desc_size >= 16` and `map_size <= 64`; a drifted `desc_size`
//! arg has no symbolic lower bound at the next entry and the recast must
//! refuse rather than inherit the stale floor.

use typed_trees::TypedTrees;

fn program(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

const STRIDE_WALK_STABLE_WITNESS: &str = "
data Desc { kind: u64; pages: u64; }
data Main {
    buf: [u8; 64];
    descriptor_size: u32 in Wrapping;
    map_size: u32 in Wrapping;
}
machine Main::main(&mut self) {
    self.descriptor_size = 16;
    self.map_size = 64;
    transition self.map_size <= 64 && self.descriptor_size >= 16 {
        true -> walk(0, self.descriptor_size, self.map_size)
        _ -> done()
    }

    state walk(
        &mut self,
        off: u32 in Wrapping,
        desc_size: u32 in Wrapping,
        map_size: u32 in Wrapping
    ) {
        let d: &Desc = &self.buf[off] as &Desc;
        transition off + desc_size + desc_size <= map_size {
            true -> walk(off + desc_size, desc_size, map_size)
            _ -> done()
        }
    }

    state done(&mut self) {}
}";

const STRIDE_WALK_DRIFTED_WITNESS: &str = "
data Desc { kind: u64; pages: u64; }
data Main {
    buf: [u8; 64];
    descriptor_size: u32 in Wrapping;
    map_size: u32 in Wrapping;
}
machine Main::main(&mut self) {
    self.descriptor_size = 16;
    self.map_size = 64;
    transition self.map_size <= 64 && self.descriptor_size >= 16 {
        true -> walk(0, self.descriptor_size, self.map_size)
        _ -> done()
    }

    state walk(
        &mut self,
        off: u32 in Wrapping,
        desc_size: u32 in Wrapping,
        map_size: u32 in Wrapping
    ) {
        let d: &Desc = &self.buf[off] as &Desc;
        transition off + desc_size + desc_size <= map_size {
            true -> walk(off + desc_size, desc_size + 1, map_size)
            _ -> done()
        }
    }

    state done(&mut self) {}
}";

#[test]
fn a_stable_stride_witness_bounds_the_dependent_view() {
    let typed = program(STRIDE_WALK_STABLE_WITNESS);
    let result = crate::validate_program(&typed);
    assert!(
        result.is_ok(),
        "self-forwarded stride witnesses prove the view footprint: {result:?}"
    );
}

#[test]
fn a_drifted_stride_witness_refuses_the_dependent_view() {
    let typed = program(STRIDE_WALK_DRIFTED_WITNESS);
    let diagnostics = crate::validate_program(&typed)
        .expect_err("a drifted desc_size cannot carry its entry floor to the next iteration");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("cannot bound the recast offset"),
        "the refusal must name the unbounded offset bound meet: {rendered}"
    );
}
