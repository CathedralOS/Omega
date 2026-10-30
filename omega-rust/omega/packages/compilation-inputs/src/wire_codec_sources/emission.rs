use std::collections::HashMap;
use std::fmt::Write;

use source::SourceMap;
use syntax_trees::SyntaxTrees;
use syntax_trees::types::TypeReferenceNode;

use super::CodecNames;
use super::analysis::{
    DomainPredicate, MemberShape, RecordShape, ScalarKind, SliceElement, StoreFacts,
    StorePredicate, WireMemberShape, const_literal_capacity, scalar_kind_of_type,
    slice_element_of_type, store_type_facts, value_field_facts, varint_bytes,
};

// Each emission helper declares an ordinary nested machine with explicit
// parameters and a terminal call to the next helper. Byte-touching helpers
// require the bounds their caller establishes; nesting supplies no facts.

struct MachineEmitter<'a> {
    text: &'a mut String,
    names: &'a CodecNames,
    /// Shared parameter declarations (`value: &V, out: &mut B, written: &mut
    /// u64, at: u64` for encode; the decode list adds `bad: u64`).
    declarations: String,
    /// Shared argument list (`(value, out, written, at)`).
    call: String,
    /// The byte-source length expression bounds are proved against.
    limit: String,
}

impl MachineEmitter<'_> {
    /// `machine name(declarations, extras) requires ... { body }`
    fn machine(&mut self, name: &str, extras: &str, requires: &str, body: &str) {
        let extras = if extras.is_empty() {
            String::new()
        } else {
            format!(", {extras}")
        };
        let requires = if requires.is_empty() {
            String::new()
        } else {
            format!(" requires {requires}")
        };
        let _ = writeln!(
            self.text,
            "    machine {name}({}{}){requires} {{\n{body}    }}\n",
            self.declarations, extras
        );
    }

    /// `machine name(declarations, extras) { transition { _ -> next(inner,
    /// extra_args) } }` — a named stage boundary hop. `extra_args` joins the
    /// shared arguments inside the parens.
    fn hop(&mut self, name: &str, extras: &str, next: &str, extra_args: &str) {
        let inner = self.call.trim_start_matches('(').trim_end_matches(')');
        self.machine(
            name,
            extras,
            "",
            &format!("        transition {{ _ -> {next}({inner}{extra_args}) }}\n"),
        );
    }

    // ----- encode helpers (thread: value, out, written, at) -----

    /// Guarded literal-byte store: `transition at < out.len { true -> put _
    /// -> overflow }`; `put` writes `out[at] = byte` behind the proved guard
    /// and continues with `at + 1`.
    fn enc_byte(&mut self, name: &str, byte: u8, next: &str) {
        let prefix = self.names.prefix.as_str();
        let put = format!("{name}_put");
        self.machine(
            name,
            "",
            "",
            &format!(
                "        transition {prefix}at < {} {{\n            true -> {put}{}\n            _ -> {prefix}encode_overflow{}\n        }}\n",
                self.limit, self.call, self.call
            ),
        );
        self.machine(
            &put,
            "",
            &format!("{prefix}at < {}", self.limit),
            &format!(
                "        {prefix}out[{prefix}at] = {byte};\n        transition {{ _ -> {next}({prefix}value, {prefix}out, {prefix}written, {prefix}at + 1) }}\n"
            ),
        );
    }

    /// One guarded store per varint tag byte: `label(index)` names each state
    /// and the chain continues to `next` after the last byte.
    fn enc_tag_bytes(&mut self, tag_bytes: &[u8], label: impl Fn(usize) -> String, next: &str) {
        for (index, byte) in tag_bytes.iter().enumerate() {
            let byte_next = byte_next_label(tag_bytes.len(), index, &label, next);
            self.enc_byte(&label(index), *byte, &byte_next);
        }
    }

    /// The decode twin of `enc_tag_bytes`: `expect_byte` per tag byte with
    /// the caller's `carry` extras threaded through each state.
    fn dec_tag_bytes(
        &mut self,
        tag_bytes: &[u8],
        label: impl Fn(usize) -> String,
        next: &str,
        carry: &str,
        carry_arg: &str,
    ) {
        for (index, byte) in tag_bytes.iter().enumerate() {
            let byte_next = byte_next_label(tag_bytes.len(), index, &label, next);
            self.dec_byte(&label(index), *byte, &byte_next, carry, carry_arg);
        }
    }

    /// A bool member encodes as one byte selected by a transition on the
    /// value itself: `name` hosts the selector state, `next` follows either
    /// store.
    fn enc_bool_member(&mut self, name: &str, value: &str, next: &str) {
        self.machine(
            name,
            "",
            "",
            &format!(
                "        transition {value} {{\n            true -> {name}_one{}\n            _ -> {name}_zero{}\n        }}\n",
                self.call, self.call
            ),
        );
        self.enc_byte(&format!("{name}_one"), 1, next);
        self.enc_byte(&format!("{name}_zero"), 0, next);
    }

    /// Canonical LEB128 emission of u64 `bits`, entering with `bits` already a
    /// state parameter (`name` is the loop state).
    fn enc_varint_loop(&mut self, name: &str, next: &str) {
        let prefix = self.names.prefix.as_str();
        let last = format!("{name}_last");
        let put = format!("{name}_put");
        let more = format!("{name}_more");
        let put_more = format!("{name}_put_more");
        self.machine(
            name,
            &format!("{prefix}bits: u64"),
            "",
            &format!(
                "        transition {prefix}bits < 128 {{\n            true -> {last}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}bits)\n            _ -> {more}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}bits)\n        }}\n"
            ),
        );
        self.machine(
            &last,
            &format!("{prefix}bits: u64"),
            "",
            &format!(
                "        let {prefix}byte: u8 = {prefix}bits as u8;\n        transition {prefix}at < {} {{\n            true -> {put}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}byte)\n            _ -> {prefix}encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.machine(
            &put,
            &format!("{prefix}byte: u8"),
            &format!("{prefix}at < {}", self.limit),
            &format!(
                "        {prefix}out[{prefix}at] = {prefix}byte;\n        transition {{ _ -> {next}({prefix}value, {prefix}out, {prefix}written, {prefix}at + 1) }}\n"
            ),
        );
        self.machine(
            &more,
            &format!("{prefix}bits: u64"),
            "",
            &format!(
                "        let {prefix}byte: u8 = (({prefix}bits & 127) | 128) as u8;\n        transition {prefix}at < {} {{\n            true -> {put_more}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}byte, {prefix}bits)\n            _ -> {prefix}encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.machine(
            &put_more,
            &format!("{prefix}byte: u8, {prefix}bits: u64"),
            &format!("{prefix}at < {}", self.limit),
            &format!(
                "        {prefix}out[{prefix}at] = {prefix}byte;\n        transition {{ _ -> {name}({prefix}value, {prefix}out, {prefix}written, {prefix}at + 1, {prefix}bits >> 7) }}\n"
            ),
        );
    }

    /// Bind the scalar payload to `bits`, then enter the varint loop.
    fn enc_varint(&mut self, name: &str, binding: &str, next: &str) {
        let prefix = self.names.prefix.as_str();
        let loop_state = format!("{name}_loop");
        self.machine(
            name,
            "",
            "",
            &format!(
                "{binding}        transition {{ _ -> {loop_state}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}bits) }}\n"
            ),
        );
        self.enc_varint_loop(&loop_state, next);
    }

    /// Varint byte-length measure of `bits`, accumulating into `acc` and
    /// continuing at `next` (which takes `acc: u64 in Wrapping`). The counter
    /// rides the `Wrapping` domain: it is a byte extent bounded by buffer
    /// extent in every reachable program, but `acc + 1` is an overflow-proof
    /// obligation the checker cannot discharge inductively, so the domain
    /// names its totality instead of a bound.
    fn enc_measure(&mut self, name: &str, binding: &str, next: &str) {
        let prefix = self.names.prefix.as_str();
        let loop_state = format!("{name}_loop");
        self.machine(
            name,
            &format!("{prefix}accumulator: u64 in Wrapping"),
            "",
            &format!(
                "{binding}        transition {{ _ -> {loop_state}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}accumulator, {prefix}bits) }}\n"
            ),
        );
        self.machine(
            &loop_state,
            &format!("{prefix}accumulator: u64 in Wrapping, {prefix}bits: u64"),
            "",
            &format!(
                "        transition {prefix}bits < 128 {{\n            true -> {next}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}accumulator + 1)\n            _ -> {loop_state}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}accumulator + 1, {prefix}bits >> 7)\n        }}\n"
            ),
        );
    }

    /// Byte-for-byte copy of a `&[u8]` member into the buffer: `i` walks the
    /// view while `at` walks the output. Requires on a callee must be
    /// established by a guard in its immediate caller, so the element-read
    /// and store bounds are re-tested in the state right before the call.
    fn enc_slice_bytes(&mut self, name: &str, field: &str, next: &str) {
        let prefix = self.names.prefix.as_str();
        let walk = format!("{name}_walk");
        let bound = format!("{name}_bound");
        let put = format!("{name}_put");
        // `requires` only admits parameter operands, so the member is seeded
        // as a `view` local at the entry state: `i < view.len` is a
        // parameter-rooted term the contract namespace admits, and `view[i]`
        // proves in `put` without any member-path correspondence.
        self.machine(
            name,
            "",
            "",
            &format!(
                "        let {prefix}view: &[u8] = {prefix}value.{field};\n        transition {{ _ -> {walk}({prefix}value, {prefix}out, {prefix}written, {prefix}at, 0, {prefix}view) }}\n"
            ),
        );
        self.machine(
            &walk,
            &format!("{prefix}index: u64, {prefix}view: &[u8]"),
            "",
            &format!(
                "        transition {prefix}index < {prefix}view.len {{\n            true -> {bound}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}view)\n            _ -> {next}({prefix}value, {prefix}out, {prefix}written, {prefix}at)\n        }}\n"
            ),
        );
        self.machine(
            &bound,
            &format!("{prefix}index: u64, {prefix}view: &[u8]"),
            "",
            &format!(
                "        transition {prefix}at < {} && {prefix}index < {prefix}view.len {{\n            true -> {put}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}view)\n            _ -> {prefix}encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.machine(
            &put,
            &format!("{prefix}index: u64, {prefix}view: &[u8]"),
            &format!("{prefix}at < {} && {prefix}index < {prefix}view.len", self.limit),
            &format!(
                "        {prefix}out[{prefix}at] = {prefix}view[{prefix}index];\n        transition {{ _ -> {walk}({prefix}value, {prefix}out, {prefix}written, {prefix}at + 1, {prefix}index + 1, {prefix}view) }}\n"
            ),
        );
    }

    /// Packed element varints of a borrowed `&[E]` member (E wider than
    /// `u8`): measure the elements' total varint extent, publish it as the
    /// byte length, then emit the elements. `i` walks the view and `acc` the
    /// measured extent; the view's `.len` is re-read at each loop head so
    /// the index evidence stays the same name the element reads use.
    fn enc_packed_slice(&mut self, name: &str, field: &str, element: ScalarKind, next: &str) {
        let prefix = self.names.prefix.as_str();
        self.enc_packed(
            name,
            &format!("{prefix}value.{field}.len"),
            &format!("{prefix}value.{field}"),
            None,
            element,
            next,
        );
    }

    /// Packed element varints behind a byte-length varint, shared by
    /// borrowed-slice (`len` elements at `value.f[i]`) and `FixedVec`
    /// (`length` elements at `value.f.items[i]`, index-bounded by the
    /// constant `bound` capacity) members.
    fn enc_packed(
        &mut self,
        name: &str,
        len_expr: &str,
        items_expr: &str,
        bound_expr: Option<&str>,
        element: ScalarKind,
        next: &str,
    ) {
        let prefix = self.names.prefix.as_str();
        // `requires` predicates only admit parameter operands, never a
        // member-path read like `value.readings.len` or
        // `value.samples.length`. `FixedVec` seeds a `slen` local; slices
        // instead seed a `view` local bound to the member itself, so every
        // guard and requires spells `i < view.len` — a parameter-rooted
        // term the contract namespace admits — and `view[i]` reads are
        // provable inside each callee without any correspondence.
        let seed_len = bound_expr.is_some();
        let member_expr = items_expr;
        let (index_guard, loop_guard, index_requires, carry, carry_arg, items_expr) =
            match bound_expr {
                Some(bound) => (
                    format!("{prefix}index < {prefix}element_count && {prefix}index < {bound}"),
                    format!("{prefix}index < {prefix}element_count"),
                    format!("{prefix}index < {prefix}element_count && {prefix}index < {bound}"),
                    format!(", {prefix}element_count: u64"),
                    format!(", {prefix}element_count"),
                    member_expr.to_string(),
                ),
                None => (
                    format!("{prefix}index < {prefix}view.len"),
                    format!("{prefix}index < {prefix}view.len"),
                    format!("{prefix}index < {prefix}view.len"),
                    format!(", {prefix}view: &[{}]", element.name()),
                    format!(", {prefix}view"),
                    format!("{prefix}view"),
                ),
            };
        let measure_entry = format!("{name}_measure");
        let measure_elem = format!("{name}_measure_elem");
        let measure_varint = format!("{name}_measure_varint");
        let length = format!("{name}_length");
        let emit_entry = format!("{name}_emit");
        let emit_entry_target = format!("{name}_emit_seed");
        let emit_elem = format!("{name}_emit_elem");
        let emit_varint = format!("{name}_emit_varint");
        let emit_last = format!("{name}_emit_last");
        let emit_last_put = format!("{name}_emit_last_put");
        let emit_more = format!("{name}_emit_more");
        let emit_more_put = format!("{name}_emit_more_put");
        let bits_binding =
            |i: &str| element.encode_bits_binding(&format!("{items_expr}[{i}]"), prefix);
        // `FixedVec` seeds `slen` (`value.samples.length` is a member path
        // no predicate admits); slices seed `view` from the member itself —
        // `i < view.len` is parameter-rooted everywhere it rides. The bound
        // checks live only at the two entry states; the inner varint byte
        // loops re-enter through them, so `bits < 128` alone decides.
        if seed_len {
            let seed = format!("{name}_seed");
            self.machine(
                name,
                "",
                "",
                &format!(
                    "        let {prefix}element_count: u64 = {len_expr};\n        transition {{ _ -> {seed}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}element_count) }}\n"
                ),
            );
            self.hop(
                &seed,
                &format!("{prefix}element_count: u64"),
                &measure_entry,
                &format!(", 0, 0, {prefix}element_count"),
            );
        } else {
            self.machine(
                name,
                "",
                "",
                &format!(
                    "        let {prefix}view: &[{}] = {member_expr};\n        transition {{ _ -> {measure_entry}({prefix}value, {prefix}out, {prefix}written, {prefix}at, 0, 0, {prefix}view) }}\n",
                    element.name()
                ),
            );
        }
        self.machine(
            &measure_entry,
            &format!("{prefix}index: u64, {prefix}accumulator: u64 in Wrapping{carry}"),
            "",
            &format!(
                "        transition {index_guard} {{\n            true -> {measure_elem}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}accumulator{carry_arg})\n            _ -> {length}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}accumulator)\n        }}\n"
            ),
        );
        self.machine(
            &measure_elem,
            &format!("{prefix}index: u64, {prefix}accumulator: u64 in Wrapping{carry}"),
            &index_requires,
            &format!(
                "{}        transition {{ _ -> {measure_varint}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}accumulator, {prefix}bits{carry_arg}) }}\n",
                bits_binding(&format!("{prefix}index"))
            ),
        );
        self.machine(
            &measure_varint,
            &format!("{prefix}index: u64, {prefix}accumulator: u64 in Wrapping, {prefix}bits: u64{carry}"),
            &loop_guard,
            &format!(
                "        transition {prefix}bits < 128 && {loop_guard} {{\n            true -> {measure_entry}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index + 1, {prefix}accumulator + 1{carry_arg})\n            _ -> {measure_varint}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}accumulator + 1, {prefix}bits >> 7{carry_arg})\n        }}\n"
            ),
        );
        self.machine(
            &length,
            &format!("{prefix}index: u64, {prefix}accumulator: u64 in Wrapping"),
            "",
            &format!(
                "        let {prefix}bits: u64 = {prefix}accumulator as u64;\n        transition {{ _ -> {length}_loop({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}bits) }}\n"
            ),
        );
        // The byte-length varint: the canonical last/more loop with `i`
        // riding as an inert carry so the emit half starts over at 0.
        {
            let last = format!("{length}_loop_last");
            let put = format!("{length}_loop_put");
            let more = format!("{length}_loop_more");
            let put_more = format!("{length}_loop_put_more");
            let loop_name = format!("{length}_loop");
            self.machine(
                &loop_name,
                &format!("{prefix}index: u64, {prefix}bits: u64"),
                "",
                &format!(
                    "        transition {prefix}bits < 128 {{\n            true -> {last}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}bits)\n            _ -> {more}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}bits)\n        }}\n"
                ),
            );
            self.machine(
                &last,
                &format!("{prefix}index: u64, {prefix}bits: u64"),
                "",
                &format!(
                    "        let {prefix}byte: u8 = {prefix}bits as u8;\n        transition {prefix}at < {} {{\n            true -> {put}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}byte)\n            _ -> {prefix}encode_overflow{}\n        }}\n",
                    self.limit, self.call
                ),
            );
            self.machine(
                &put,
                &format!("{prefix}index: u64, {prefix}byte: u8"),
                &format!("{prefix}at < {}", self.limit),
                &format!(
                    "        {prefix}out[{prefix}at] = {prefix}byte;\n        transition {{ _ -> {emit_entry_target}({prefix}value, {prefix}out, {prefix}written, {prefix}at + 1, 0) }}\n"
                ),
            );
            self.machine(
                &more,
                &format!("{prefix}index: u64, {prefix}bits: u64"),
                "",
                &format!(
                    "        let {prefix}byte: u8 = (({prefix}bits & 127) | 128) as u8;\n        transition {prefix}at < {} {{\n            true -> {put_more}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}byte, {prefix}bits)\n            _ -> {prefix}encode_overflow{}\n        }}\n",
                    self.limit, self.call
                ),
            );
            self.machine(
                &put_more,
                &format!("{prefix}index: u64, {prefix}byte: u8, {prefix}bits: u64"),
                &format!("{prefix}at < {}", self.limit),
                &format!(
                    "        {prefix}out[{prefix}at] = {prefix}byte;\n        transition {{ _ -> {loop_name}({prefix}value, {prefix}out, {prefix}written, {prefix}at + 1, {prefix}index, {prefix}bits >> 7) }}\n"
                ),
            );
        }
        // The emit half re-seeds the iteration bound under each member
        // kind — `slen` for `FixedVec`, `view` for slices — so the
        // length-loop tail stays thin.
        if bound_expr.is_some() {
            self.machine(
                &emit_entry_target,
                &format!("{prefix}index: u64"),
                "",
                &format!(
                    "        let {prefix}element_count: u64 = {len_expr};\n        transition {{ _ -> {emit_entry}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}element_count) }}\n"
                ),
            );
        } else {
            self.machine(
                &emit_entry_target,
                &format!("{prefix}index: u64"),
                "",
                &format!(
                    "        let {prefix}view: &[{}] = {member_expr};\n        transition {{ _ -> {emit_entry}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}view) }}\n",
                    element.name()
                ),
            );
        }
        self.machine(
            &emit_entry,
            &format!("{prefix}index: u64{carry}"),
            "",
            &format!(
                "        transition {index_guard} {{\n            true -> {emit_elem}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index{carry_arg})\n            _ -> {next}({prefix}value, {prefix}out, {prefix}written, {prefix}at)\n        }}\n"
            ),
        );
        self.machine(
            &emit_elem,
            &format!("{prefix}index: u64{carry}"),
            &index_requires,
            &format!(
                "{}        transition {{ _ -> {emit_varint}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}bits{carry_arg}) }}\n",
                bits_binding(&format!("{prefix}index"))
            ),
        );
        self.machine(
            &emit_varint,
            &format!("{prefix}index: u64, {prefix}bits: u64{carry}"),
            &loop_guard,
            &format!(
                "        transition {prefix}bits < 128 {{\n            true -> {emit_last}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}bits{carry_arg})\n            _ -> {emit_more}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}bits{carry_arg})\n        }}\n"
            ),
        );
        self.machine(
            &emit_last,
            &format!("{prefix}index: u64, {prefix}bits: u64{carry}"),
            &loop_guard,
            &format!(
                "        let {prefix}byte: u8 = {prefix}bits as u8;\n        transition {prefix}at < {} && {loop_guard} {{\n            true -> {emit_last_put}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}byte{carry_arg})\n            _ -> {prefix}encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.machine(
            &emit_last_put,
            &format!("{prefix}index: u64, {prefix}byte: u8{carry}"),
            &format!("{prefix}at < {} && {loop_guard}", self.limit),
            &format!(
                "        {prefix}out[{prefix}at] = {prefix}byte;\n        transition {{ _ -> {emit_entry}({prefix}value, {prefix}out, {prefix}written, {prefix}at + 1, {prefix}index + 1{carry_arg}) }}\n"
            ),
        );
        self.machine(
            &emit_more,
            &format!("{prefix}index: u64, {prefix}bits: u64{carry}"),
            &loop_guard,
            &format!(
                "        let {prefix}byte: u8 = (({prefix}bits & 127) | 128) as u8;\n        transition {prefix}at < {} && {loop_guard} {{\n            true -> {emit_more_put}({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}index, {prefix}byte, {prefix}bits{carry_arg})\n            _ -> {prefix}encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.machine(
            &emit_more_put,
            &format!("{prefix}index: u64, {prefix}byte: u8, {prefix}bits: u64{carry}"),
            &format!("{prefix}at < {} && {loop_guard}", self.limit),
            &format!(
                "        {prefix}out[{prefix}at] = {prefix}byte;\n        transition {{ _ -> {emit_varint}({prefix}value, {prefix}out, {prefix}written, {prefix}at + 1, {prefix}index, {prefix}bits >> 7{carry_arg}) }}\n"
            ),
        );
    }

    /// A nested-message member's sub-body: the byte length is the static
    /// tag bytes plus each non-bool member's varint extent measured in
    /// `acc`; the body then emits the nested schema's own tag+content
    /// sequence with no era. `member_facts` admits scalar-only sub-members,
    /// so a bool content is the branch byte and everything else a varint.
    fn enc_nested(&mut self, name: &str, field: &str, sub_members: &[WireMemberShape], next: &str) {
        let prefix = self.names.prefix.as_str();
        let measure_stage = format!("{name}_measure");
        let length = format!("{name}_length");
        let mut static_bytes: u64 = 0;
        let mut measured: Vec<&WireMemberShape> = Vec::new();
        for sub in sub_members {
            static_bytes += varint_bytes(sub.number).len() as u64;
            match &sub.shape {
                MemberShape::Scalar(ScalarKind::Bool) => static_bytes += 1,
                MemberShape::Scalar(_) => measured.push(sub),
                _ => continue,
            }
        }
        let first = if measured.is_empty() {
            length.clone()
        } else {
            measure_stage.clone()
        };
        self.hop(name, "", &first, &format!(", {static_bytes}"));
        for (index, sub) in measured.iter().enumerate() {
            let label = if index == 0 {
                measure_stage.clone()
            } else {
                format!("{name}_measure_{index}")
            };
            let measure_next = if index + 1 == measured.len() {
                length.clone()
            } else {
                format!("{name}_measure_{}", index + 1)
            };
            let MemberShape::Scalar(kind) = &sub.shape else {
                continue;
            };
            self.enc_measure(
                &label,
                &kind.encode_bits_binding(&format!("{prefix}value.{field}.{}", sub.name), prefix),
                &measure_next,
            );
        }
        let first_sub = format!("{name}_subtag_0_0");
        let sub_next = |index: usize| {
            if index + 1 == sub_members.len() {
                next.to_string()
            } else {
                format!("{name}_subtag_{}_0", index + 1)
            }
        };
        self.machine(
            &length,
            &format!("{prefix}accumulator: u64 in Wrapping"),
            "",
            &format!(
                "        let {prefix}bits: u64 = {prefix}accumulator as u64;\n        transition {{ _ -> {length}_loop({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}bits) }}\n"
            ),
        );
        self.enc_varint_loop(&format!("{length}_loop"), &first_sub);
        for (sub_index, sub) in sub_members.iter().enumerate() {
            let after_sub = sub_next(sub_index);
            let tag_bytes = varint_bytes(sub.number);
            let content = format!("{name}_subcontent_{sub_index}");
            self.enc_tag_bytes(
                &tag_bytes,
                |index| format!("{name}_subtag_{sub_index}_{index}"),
                &content,
            );
            let MemberShape::Scalar(kind) = &sub.shape else {
                continue;
            };
            if *kind == ScalarKind::Bool {
                self.enc_bool_member(
                    &content,
                    &format!("{prefix}value.{field}.{}", sub.name),
                    &after_sub,
                );
            } else {
                self.enc_varint(
                    &content,
                    &kind.encode_bits_binding(
                        &format!("{prefix}value.{field}.{}", sub.name),
                        prefix,
                    ),
                    &after_sub,
                );
            }
        }
    }

    // ----- decode helpers (thread: value, buffer, read, verdict, at, bad) -----

    /// `expect_byte`: `buffer[at]` must equal `byte`. OOB flags `bad` without
    /// consuming; a mismatch consumes and flags. `carry`/`carry_arg` thread
    /// extra state (the repeated-body `end`) through every state.
    fn dec_byte(&mut self, name: &str, byte: u8, next: &str, carry: &str, carry_arg: &str) {
        let prefix = self.names.prefix.as_str();
        let check = format!("{name}_check");
        let oob = format!("{name}_oob");
        let inner = self.call.trim_start_matches('(').trim_end_matches(')');
        self.machine(
            name,
            carry,
            "",
            &format!(
                "        transition {prefix}at < {} {{\n            true -> {check}({inner}{carry_arg})\n            _ -> {oob}({inner}{carry_arg})\n        }}\n",
                self.limit
            ),
        );
        self.machine(
            &check,
            carry,
            &format!("{prefix}at < {}", self.limit),
            &format!(
                "        transition {prefix}buffer[{prefix}at] == {byte} {{\n            true -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at + 1, {prefix}bad{carry_arg})\n            _ -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at + 1, {prefix}bad + 1{carry_arg})\n        }}\n"
            ),
        );
        self.machine(
            &oob,
            carry,
            "",
            &format!(
                "        transition {{ _ -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1{carry_arg}) }}\n"
            ),
        );
    }

    /// Canonical LEB128 read mirroring the interpreter loop: the `shift > 63`
    /// guard runs before the byte read (no consumption), OOB truncation is
    /// not consumed, and a consumed byte accumulates `payload << shift` with
    /// the tenth-payload and overlong-terminal checks folding into `bad`.
    /// `name` is the entry (it only ever receives `carry` extras); `next`
    /// receives `carry` then the accumulated `raw`.
    fn dec_varint(&mut self, name: &str, next: &str, carry: &str, carry_arg: &str) {
        let prefix = self.names.prefix.as_str();
        let start = format!("{name}_start");
        let read = format!("{name}_read");
        let byte = format!("{name}_byte");
        let terminal = format!("{name}_terminal");
        let terminal_check = format!("{name}_terminal_check");
        let continue_state = format!("{name}_continue");
        let acc_shift = join_extras(
            &format!("{prefix}accumulator: u64 in Wrapping, {prefix}shift: u64 in Wrapping"),
            carry,
            "",
        );
        let full = join_extras(
            &format!("{prefix}accumulator: u64 in Wrapping, {prefix}shift: u64 in Wrapping"),
            carry,
            &format!(", {prefix}payload: u64 in Wrapping"),
        );
        self.machine(
            name,
            carry,
            "",
            &format!(
                "        transition {{ _ -> {start}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, 0 as u64 in Wrapping, 0 as u64 in Wrapping{carry_arg}) }}\n"
            ),
        );
        self.machine(
            &start,
            &acc_shift,
            "",
            &format!(
                "        transition {prefix}shift > 63 {{\n            true -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1{carry_arg}, {prefix}accumulator as u64)\n            _ -> {read}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}accumulator, {prefix}shift{carry_arg})\n        }}\n"
            ),
        );
        self.machine(
            &read,
            &acc_shift,
            "",
            &format!(
                "        transition {prefix}at < {} {{\n            true -> {byte}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}accumulator, {prefix}shift{carry_arg})\n            _ -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1{carry_arg}, {prefix}accumulator as u64)\n        }}\n",
                self.limit
            ),
        );
        self.machine(
            &byte,
            &acc_shift,
            &format!("{prefix}at < {}", self.limit),
            &format!(
                "        let {prefix}byte: u8 = {prefix}buffer[{prefix}at];\n        let {prefix}payload: u64 in Wrapping = ({prefix}byte & 127) as u64 in Wrapping;\n        let {prefix}next_accumulator: u64 in Wrapping = {prefix}accumulator | ({prefix}payload << {prefix}shift);\n        transition {prefix}byte < 128 {{\n            true -> {terminal}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at + 1, {prefix}bad, {prefix}next_accumulator, {prefix}shift{carry_arg}, {prefix}payload)\n            _ -> {continue_state}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at + 1, {prefix}bad, {prefix}next_accumulator, {prefix}shift{carry_arg}, {prefix}payload)\n        }}\n"
            ),
        );
        self.machine(
            &terminal,
            &full,
            "",
            &format!(
                "        transition {prefix}payload == 0 && {prefix}shift > 0 {{\n            true -> {terminal_check}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1, {prefix}accumulator, {prefix}shift{carry_arg}, {prefix}payload)\n            _ -> {terminal_check}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}accumulator, {prefix}shift{carry_arg}, {prefix}payload)\n        }}\n"
            ),
        );
        self.machine(
            &terminal_check,
            &full,
            "",
            &format!(
                "        transition {prefix}shift == 63 && {prefix}payload > 1 {{\n            true -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1{carry_arg}, {prefix}accumulator as u64)\n            _ -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad{carry_arg}, {prefix}accumulator as u64)\n        }}\n"
            ),
        );
        self.machine(
            &continue_state,
            &full,
            "",
            &format!(
                "        transition {prefix}shift == 63 && {prefix}payload > 1 {{\n            true -> {start}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1, {prefix}accumulator, {prefix}shift + 7{carry_arg})\n            _ -> {start}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}accumulator, {prefix}shift + 7{carry_arg})\n        }}\n"
            ),
        );
    }

    /// Primitive landing and owner validity are distinct obligations. First
    /// prove the decoded wide value fits its carrier, then form that exact
    /// typed candidate and evaluate every retained predicate. The write helper
    /// receives the same candidate and guard as ordinary source contracts;
    /// failure forwards the unchanged valid destination with sticky `bad`.
    fn dec_scalar_store(
        &mut self,
        name: &str,
        field: &str,
        kind: ScalarKind,
        next: &str,
        facts: &[StorePredicate],
        carry: &str,
        carry_arg: &str,
    ) {
        let prefix = self.names.prefix.as_str();
        let landing = format!("{name}_candidate");
        let store = format!("{name}_write");
        let operand = if kind.decodes_signed() {
            &format!("{prefix}wide")
        } else {
            &format!("{prefix}raw")
        };
        let wide_type = if kind.decodes_signed() { "i64" } else { "u64" };
        let fit = kind.decode_range_guard(operand).unwrap_or_default();
        let arguments = format!(
            "{prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad{carry_arg}"
        );
        let failure = format!(
            "{next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1{carry_arg})"
        );
        let entry = if fit.is_empty() {
            format!("        transition {{ _ -> {landing}({arguments}, {operand}) }}\n")
        } else {
            format!(
                "        transition {fit} {{\n            true -> {landing}({arguments}, {operand})\n            _ -> {failure}\n        }}\n"
            )
        };
        let signed_binding = if kind.decodes_signed() {
            ScalarKind::decode_signed_binding(prefix)
        } else {
            String::new()
        };
        self.machine(
            name,
            &carried_operand(carry, &format!("{prefix}raw: u64")),
            "",
            &format!("{signed_binding}{entry}"),
        );

        let requires = facts
            .iter()
            .map(|fact| format!("({})", fact.instantiate(&format!("{prefix}candidate"))))
            .collect::<Vec<_>>()
            .join(" && ");
        let admission = if requires.is_empty() {
            format!("        transition {{ _ -> {store}({arguments}, {prefix}candidate) }}\n")
        } else {
            format!(
                "        transition {requires} {{\n            true -> {store}({arguments}, {prefix}candidate)\n            _ -> {failure}\n        }}\n"
            )
        };
        self.machine(
            &landing,
            &carried_operand(carry, &format!("{operand}: {wide_type}")),
            &fit,
            &format!(
                "        let {prefix}candidate: {} = {};\n{admission}",
                kind.name(),
                kind.decode_store_expression(operand)
            ),
        );
        self.machine(
            &store,
            &carried_operand(carry, &format!("{prefix}candidate: {}", kind.name())),
            &requires,
            &format!("        {prefix}value.{field} = {prefix}candidate;\n        transition {{ _ -> {next}({arguments}) }}\n"),
        );
    }

    /// Fixed-index stores use the same typed admission and commit operation.
    /// Only the destination projection and carried framing cursor differ.
    fn dec_element_store(
        &mut self,
        name: &str,
        field: &str,
        index: usize,
        kind: ScalarKind,
        next: &str,
        facts: &[StorePredicate],
    ) {
        let prefix = self.names.prefix.as_str();
        self.dec_scalar_store(
            name,
            &format!("{field}[{index}]"),
            kind,
            next,
            facts,
            &format!("{prefix}end: u64"),
            &format!(", {prefix}end"),
        );
    }

    /// The `end`-window derivation shared by every length-delimited member: a
    /// byte-length varint opens the window and `end` comes out of complement
    /// subtraction through named locals, never `at + raw` — one named local
    /// per state so each subtraction's bound is a requires premise, since
    /// arithmetic call arguments cannot carry it. On any guard failure the
    /// content's `first` state is entered with `bad + 1`.
    /// The three bound-establishing states every decode window opens with:
    /// `open` tests the cursor premise, `open_in` derives `room`, `slack`
    /// derives `slack = room - raw` and re-tests it. `next` receives
    /// `(slack, room)`; `skip` is the bad-byte fallthrough.
    fn window_open_states(
        &mut self,
        open: &str,
        open_in: &str,
        slack: &str,
        next: &str,
        skip: &str,
    ) {
        let prefix = self.names.prefix.as_str();
        self.machine(
            open,
            &format!("{prefix}raw: u64"),
            "",
            &format!(
                "        transition {prefix}at <= {} {{\n            true -> {open_in}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}raw)\n            _ -> {skip}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1, 0)\n        }}\n",
                self.limit
            ),
        );
        self.machine(
            open_in,
            &format!("{prefix}raw: u64"),
            &format!("{prefix}at <= {}", self.limit),
            &format!(
                "        let {prefix}room: u64 = {prefix}buffer.len - {prefix}at;\n        transition {prefix}raw <= {prefix}room && {prefix}room <= {prefix}buffer.len {{\n            true -> {slack}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}raw, {prefix}room)\n            _ -> {skip}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1, 0)\n        }}\n"
            ),
        );
        self.machine(
            slack,
            &format!("{prefix}raw: u64, {prefix}room: u64"),
            &format!("{prefix}raw <= {prefix}room && {prefix}room <= {}", self.limit),
            &format!(
                "        let {prefix}slack: u64 = {prefix}room - {prefix}raw;\n        transition {prefix}slack <= {prefix}room && {prefix}slack <= {prefix}buffer.len && {prefix}room <= {prefix}buffer.len {{\n            true -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}slack, {prefix}room)\n            _ -> {skip}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1, 0)\n        }}\n"
            ),
        );
    }

    fn dec_open_window(&mut self, name: &str, first: &str) {
        let prefix = self.names.prefix.as_str();
        let open = format!("{name}_open");
        let open_in = format!("{name}_open_in");
        let slack_state = format!("{name}_slack");
        let end_state = format!("{name}_end");
        self.dec_varint(name, &open, "", "");
        self.window_open_states(&open, &open_in, &slack_state, &end_state, first);
        self.machine(
            &end_state,
            &format!("{prefix}slack: u64, {prefix}room: u64"),
            &format!("{prefix}slack <= {prefix}room && {prefix}slack <= {} && {prefix}room <= {}", self.limit, self.limit),
            &format!(
                "        let {prefix}end: u64 = {prefix}buffer.len - {prefix}slack;\n        transition {{ _ -> {first}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}end) }}\n"
            ),
        );
    }

    /// The close of every `end` window: the cursor must land exactly on the
    /// bound or `bad` increments on the way to `next`.
    fn dec_close_window(&mut self, name: &str, next: &str) {
        let prefix = self.names.prefix.as_str();
        self.machine(
            &format!("{name}_close"),
            &format!("{prefix}end: u64"),
            "",
            &format!(
                "        transition {prefix}at == {prefix}end {{\n            true -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad)\n            _ -> {next}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1)\n        }}\n"
            ),
        );
    }

    /// `FixedVec<E, N>` member: a byte-length varint opens the `end` window,
    /// then each capacity slot reads an element while bytes remain — element
    /// `k` runs only behind `at < end`, so a shorter body lands `length = k`
    /// and the close still proves `at == end`. A body longer than capacity
    /// leaves `at < end` at the close and flags `bad` through it.
    fn dec_fixed_vec(
        &mut self,
        name: &str,
        field: &str,
        element: ScalarKind,
        capacity: usize,
        next: &str,
        facts: &[StorePredicate],
    ) {
        let prefix = self.names.prefix.as_str();
        let close = format!("{name}_close");
        let first = format!("{name}_element_0");
        self.dec_open_window(name, &first);
        for index in 0..capacity {
            let element_state = format!("{name}_element_{index}");
            let read = format!("{name}_element_{index}_read");
            let store = format!("{name}_element_{index}_store");
            let length_state = format!("{name}_length_{index}");
            let after_element = format!("{name}_element_{}", index + 1);
            self.machine(
                &element_state,
                &format!("{prefix}end: u64"),
                "",
                &format!(
                    "        transition {prefix}at < {prefix}end {{\n            true -> {read}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}end)\n            _ -> {length_state}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}end)\n        }}\n"
                ),
            );
            self.dec_varint(
                &read,
                &store,
                &format!("{prefix}end: u64"),
                &format!(", {prefix}end"),
            );
            self.dec_element_store(
                &store,
                &format!("{field}.items"),
                index,
                element,
                &after_element,
                facts,
            );
        }
        for index in 0..=capacity {
            let length_state = if index == capacity {
                format!("{name}_element_{capacity}")
            } else {
                format!("{name}_length_{index}")
            };
            self.machine(
                &length_state,
                &format!("{prefix}end: u64"),
                "",
                &format!(
                    "        {prefix}value.{field}.length = {index};\n        transition {{ _ -> {close}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}end) }}\n"
                ),
            );
        }
        self.dec_close_window(name, next);
    }

    /// A nested-message member: a byte-length varint opens the `end` window,
    /// the nested schema's own tag+scalar sequence decodes inside it with
    /// `end` carried through every state, and the close flags `bad` unless
    /// the cursor lands exactly on the bound.
    fn dec_nested(
        &mut self,
        name: &str,
        field: &str,
        sub_members: &[WireMemberShape],
        sub_facts: &[Vec<StorePredicate>],
        next: &str,
    ) {
        let prefix = self.names.prefix.as_str();
        let close = format!("{name}_close");
        let first = format!("{name}_subtag_0_0");
        self.dec_open_window(name, &first);
        for (sub_index, sub) in sub_members.iter().enumerate() {
            let content = format!("{name}_subcontent_{sub_index}");
            let store = format!("{name}_substore_{sub_index}");
            let tag_bytes = varint_bytes(sub.number);
            self.dec_tag_bytes(
                &tag_bytes,
                |index| format!("{name}_subtag_{sub_index}_{index}"),
                &content,
                &format!("{prefix}end: u64"),
                &format!(", {prefix}end"),
            );
            let after_sub = if sub_index + 1 == sub_members.len() {
                close.clone()
            } else {
                format!("{name}_subtag_{}_0", sub_index + 1)
            };
            self.dec_varint(
                &content,
                &store,
                &format!("{prefix}end: u64"),
                &format!(", {prefix}end"),
            );
            let MemberShape::Scalar(kind) = &sub.shape else {
                continue;
            };
            self.dec_scalar_store(
                &store,
                &format!("{field}.{}", sub.name),
                *kind,
                &after_sub,
                &sub_facts[sub_index],
                &format!("{prefix}end: u64"),
                &format!(", {prefix}end"),
            );
        }
        self.dec_close_window(name, next);
    }
}

fn byte_next_label(
    len: usize,
    index: usize,
    label: &impl Fn(usize) -> String,
    next: &str,
) -> String {
    if index + 1 == len {
        next.to_string()
    } else {
        label(index + 1)
    }
}

impl ScalarKind {
    /// Bind one scalar's canonical unsigned varint payload. Signed values
    /// widen exactly before a total same-width storage view reads their bits;
    /// zigzag's shift and sign mask are explicit modular u64 operations.
    /// Every scalar storage shape uses this binding, including measurement.
    fn encode_bits_binding(self, member: &str, prefix: &str) -> String {
        match self {
            Self::U32 | Self::Bool => format!("        let {prefix}bits: u64 = {member} as u64;\n"),
            Self::U64 => format!("        let {prefix}bits: u64 = {member};\n"),
            Self::I32 | Self::I64 => format!(
                "        let {prefix}number: i64 = {member} as i64;\n        let {prefix}representation: &u64 = &{prefix}number as &u64;\n        let {prefix}raw: u64 in Wrapping = {prefix}representation as u64 in Wrapping;\n        let {prefix}zero: u64 in Wrapping = 0;\n        let {prefix}sign_mask: u64 in Wrapping = {prefix}zero - ({prefix}raw >> 63);\n        let {prefix}bits: u64 = (({prefix}raw << 1) ^ {prefix}sign_mask) as u64;\n"
            ),
        }
    }

    /// Inverse zigzag reads the shifted payload through the total i64 view,
    /// then complements it for an odd payload. These are representation and
    /// bit operations; no numeric cast changes a denoted signed value.
    fn decode_signed_binding(prefix: &str) -> String {
        format!(
            "        let {prefix}half: u64 = {prefix}raw >> 1;\n        let {prefix}representation: &i64 = &{prefix}half as &i64;\n        let {prefix}signed_half: i64 = {prefix}representation;\n        let {prefix}low: i64 = ({prefix}raw & 1) as i64;\n        let {prefix}mask: i64 = 0 - {prefix}low;\n        let {prefix}wide: i64 = {prefix}signed_half ^ {prefix}mask;\n"
        )
    }

    fn decodes_signed(self) -> bool {
        matches!(self, Self::I32 | Self::I64)
    }
}

/// Extras list for a carried state: `{carry}, {operand}` when a carry
/// threads through, else just `{operand}`.
fn carried_operand(carry: &str, operand: &str) -> String {
    if carry.is_empty() {
        operand.to_string()
    } else {
        format!("{carry}, {operand}")
    }
}

/// Join extra parameter declarations: `base` then `carry` then `more`.
fn join_extras(base: &str, carry: &str, more: &str) -> String {
    let mut joined = base.to_string();
    if !carry.is_empty() {
        joined.push_str(", ");
        joined.push_str(carry);
    }
    joined.push_str(more);
    joined
}

/// The value type named by a signature's first argument (`&V`/`&mut V`).
fn signature_value_type(signature: &[String]) -> &str {
    signature[0]
        .trim_start_matches('&')
        .trim_start()
        .trim_start_matches("mut")
        .trim_start()
}

/// Pre-pass over members: the value type's declared field must carry the
/// member's scalar (or element scalar) shape, and the domain/range facts it
/// constrains retain their subject for the store guard.
/// `None` marks mismatched storage. Unavailable predicates stay as per-member
/// errors so encoding can use the shape while decoding declines before emission.
fn member_facts(
    names: &CodecNames,
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    records: &HashMap<String, RecordShape>,
    schemas: &[(String, Vec<WireMemberShape>)],
    value_type: &str,
    members: &[WireMemberShape],
) -> Option<Vec<StoreFacts>> {
    let prefix = names.prefix.as_str();
    let record = records.get(value_type)?;
    let value_fields = &record.fields;
    let mut all = Vec::with_capacity(members.len());
    for member in members {
        // Element stores replace a leaf, not the array or vector containing
        // it. A predicate consuming that entire aggregate needs a prospective
        // aggregate; RecordShape refuses to reuse its old value as evidence.
        let projection = match member.shape {
            MemberShape::FixedScalarArray { .. } | MemberShape::FixedVec { .. } => {
                format!("{}[index]", member.name)
            }
            _ => member.name.clone(),
        };
        let owner_facts = record.store_facts(&projection, "", &format!("{prefix}value"));
        let extend_owner = |facts: StoreFacts| {
            facts.and_then(|mut facts| {
                facts.extend(owner_facts?);
                Ok(facts)
            })
        };
        // A view's storage shape and its destination predicates are independent
        // obligations: matching &[E] cannot establish a qualified byte domain.
        if let MemberShape::BorrowedSlice { element } = &member.shape {
            let field_handle = value_fields.get(member.name.as_str()).copied()?;
            let element_name = match element {
                SliceElement::Byte => "u8",
                SliceElement::Scalar(kind) => kind.name(),
            };
            let carrier = format!("[{element_name}]");
            let (field_handle, facts) =
                store_type_facts(syntax_trees, sources, domains, field_handle, &carrier);
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = syntax_trees
                .tables
                .type_references
                .type_reference(field_handle)
            else {
                return None;
            };
            if access.is_exclusive() {
                return None;
            }
            // `&([E] in Domain)`: the slice's qualification binds inside the
            // reference; retain its predicates as well as outer qualifications.
            let (referee, referee_facts) =
                store_type_facts(syntax_trees, sources, domains, *referee, &carrier);
            let facts = facts.and_then(|mut facts| {
                facts.extend(referee_facts?);
                Ok(facts)
            });
            let TypeReferenceNode::Slice { element_type } =
                syntax_trees.tables.type_references.type_reference(referee)
            else {
                return None;
            };
            if slice_element_of_type(syntax_trees, *element_type) != Some(*element) {
                return None;
            }
            all.push(extend_owner(facts));
            continue;
        }
        // A `FixedVec<E, N>` member's value field must spell the same
        // instantiation; the element's own domain/range facts ride every
        // element store's guard exactly like a fixed array's.
        if let MemberShape::FixedVec { element, capacity } = &member.shape {
            let field_handle = value_fields.get(member.name.as_str()).copied()?;
            let TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } = syntax_trees
                .tables
                .type_references
                .type_reference(field_handle)
            else {
                return None;
            };
            let arguments = syntax_trees
                .tables
                .type_references
                .type_reference_handles(*arguments);
            if base_name.as_str(syntax_trees) != "FixedVec"
                || arguments.len() != 2
                || scalar_kind_of_type(syntax_trees, arguments[0]) != Some(*element)
                || const_literal_capacity(syntax_trees, arguments[1]) != Some(*capacity)
            {
                return None;
            }
            let facts = value_field_facts(
                syntax_trees,
                sources,
                domains,
                Some(arguments[0]),
                *element,
                None,
            )?;
            all.push(extend_owner(facts));
            continue;
        }
        // A nested-message member's value field names the value mirror of
        // the nested schema; every nested member must be a scalar whose
        // value-side field resolves — non-scalar nesting stays on the
        // intrinsic wall, and the sub-member store guards re-collect the
        // scalar facts at emission.
        if let MemberShape::NestedMessage { schema } = &member.shape {
            let field_handle = value_fields.get(member.name.as_str()).copied()?;
            let value_name = syntax_trees
                .tables
                .type_references
                .type_reference(field_handle)
                .as_named()?;
            let nested_record = records.get(value_name.as_str(syntax_trees))?;
            let nested_fields = &nested_record.fields;
            let (_, sub_members) = schemas
                .iter()
                .find(|(name, _)| name.as_str() == schema.as_str())?;
            let mut requirements = Ok(Vec::new());
            for sub in sub_members {
                let MemberShape::Scalar(kind) = &sub.shape else {
                    return None;
                };
                let facts = value_field_facts(
                    syntax_trees,
                    sources,
                    domains,
                    nested_fields.get(sub.name.as_str()).copied(),
                    *kind,
                    None,
                )?;
                // Nested guards are emitted per field, but failure to resolve
                // any predicate must decline decoding before text is emitted.
                let predicates = facts.and_then(|mut facts| {
                    facts.extend(nested_record.store_facts(
                        &sub.name,
                        &format!("{}.", member.name),
                        &format!("{prefix}value"),
                    )?);
                    facts.extend(record.store_facts(
                        &format!("{}.{}", member.name, sub.name),
                        "",
                        &format!("{prefix}value"),
                    )?);
                    Ok(facts)
                });
                requirements = requirements.and(predicates.map(|_| Vec::new()));
            }
            all.push(requirements);
            continue;
        }
        let (expected, array_length) = match &member.shape {
            MemberShape::Scalar(kind) => (*kind, None),
            MemberShape::FixedScalarArray { element, length } => (*element, Some(*length)),
            MemberShape::BorrowedSlice { .. }
            | MemberShape::FixedVec { .. }
            | MemberShape::NestedMessage { .. } => unreachable!("handled above"),
        };
        let facts = value_field_facts(
            syntax_trees,
            sources,
            domains,
            value_fields.get(member.name.as_str()).copied(),
            expected,
            array_length,
        )?;
        all.push(extend_owner(facts));
    }
    Some(all)
}

/// Per-sub-member store facts for a nested-message member: the value type's
/// `member` field names the nested value mirror; each nested scalar member's
/// facts resolve against that mirror's field map, one `Vec` per sub-member
/// for the store guards.
#[allow(clippy::too_many_arguments)]
fn nested_sub_facts(
    names: &CodecNames,
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    records: &HashMap<String, RecordShape>,
    value_type: &str,
    member_name: &str,
    schemas: &[(String, Vec<WireMemberShape>)],
    schema: &str,
) -> Option<Vec<Vec<StorePredicate>>> {
    let prefix = names.prefix.as_str();
    let record = records.get(value_type)?;
    let field_handle = record.fields.get(member_name).copied()?;
    let value_name = syntax_trees
        .tables
        .type_references
        .type_reference(field_handle)
        .as_named()?;
    let nested_record = records.get(value_name.as_str(syntax_trees))?;
    let nested_fields = &nested_record.fields;
    let (_, sub_members) = schemas.iter().find(|(name, _)| name.as_str() == schema)?;
    let mut all = Vec::with_capacity(sub_members.len());
    for sub in sub_members {
        let MemberShape::Scalar(kind) = &sub.shape else {
            return None;
        };
        let mut facts = value_field_facts(
            syntax_trees,
            sources,
            domains,
            nested_fields.get(sub.name.as_str()).copied(),
            *kind,
            None,
        )?
        .ok()?;
        facts.extend(
            nested_record
                .store_facts(
                    &sub.name,
                    &format!("{member_name}."),
                    &format!("{prefix}value"),
                )
                .ok()?,
        );
        facts.extend(
            record
                .store_facts(
                    &format!("{member_name}.{}", sub.name),
                    "",
                    &format!("{prefix}value"),
                )
                .ok()?,
        );
        all.push(facts);
    }
    Some(all)
}

/// `machine S::encode(value: &V, out: &mut B, written: &mut u64)`: the era
/// varint, then per field (number order) a tag varint and the member content,
/// ending by publishing the cursor through `written`.
pub(super) fn emit_encode_machine(
    names: &CodecNames,
    text: &mut String,
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    records: &HashMap<String, RecordShape>,
    schemas: &[(String, Vec<WireMemberShape>)],
    schema_name: &str,
    signature: &[String],
    members: &[WireMemberShape],
) {
    let prefix = names.prefix.as_str();
    if member_facts(
        names,
        syntax_trees,
        sources,
        domains,
        records,
        schemas,
        signature_value_type(signature),
        members,
    )
    .is_none()
    {
        return;
    }
    let mut emitter = MachineEmitter {
        names,
        text,
        declarations: format!(
            "{prefix}value: {}, {prefix}out: {}, {prefix}written: {}, {prefix}at: u64",
            signature[0], signature[1], signature[2]
        ),
        call: format!("({prefix}value, {prefix}out, {prefix}written, {prefix}at)"),
        limit: format!("{prefix}out.len"),
    };
    let _ = writeln!(
        emitter.text,
        "machine {schema_name}::encode({prefix}value: {}, {prefix}out: {}, {prefix}written: {}) {{\n    transition {{ _ -> {prefix}era_0({prefix}value, {prefix}out, {prefix}written, 0) }}\n",
        signature[0], signature[1], signature[2]
    );

    // Stage labels in wire order: era, then per field tag+content, then
    // finish. Each emission names its continuation.
    let mut stages: Vec<String> = Vec::new();
    for member in members {
        stages.push(format!("{prefix}field_{}_tag_0", member.number));
        stages.push(format!("{prefix}field_{}_content", member.number));
    }
    let era_next = stages
        .first()
        .cloned()
        .unwrap_or_else(|| format!("{prefix}encode_finish"));
    let era_bytes = varint_bytes(0);
    emitter.enc_tag_bytes(
        &era_bytes,
        |index| format!("{prefix}era_{index}"),
        &era_next,
    );

    for (member_index, member) in members.iter().enumerate() {
        let content_label = format!("{prefix}field_{}_content", member.number);
        let after = if member_index + 1 == members.len() {
            format!("{prefix}encode_finish")
        } else {
            format!("{prefix}field_{}_tag_0", members[member_index + 1].number)
        };
        let tag_bytes = varint_bytes(member.number);
        emitter.enc_tag_bytes(
            &tag_bytes,
            |index| format!("{prefix}field_{}_tag_{index}", member.number),
            &content_label,
        );
        match &member.shape {
            MemberShape::Scalar(ScalarKind::Bool) => {
                emitter.enc_bool_member(
                    &content_label,
                    &format!("{prefix}value.{}", member.name),
                    &after,
                );
            }
            MemberShape::Scalar(kind) => {
                let bits =
                    kind.encode_bits_binding(&format!("{prefix}value.{}", member.name), prefix);
                emitter.enc_varint(&content_label, &bits, &after);
            }
            MemberShape::BorrowedSlice { element } => {
                // Each sequence has one byte-length prefix. For raw bytes
                // this is the view extent; packed scalars measure their
                // encoded extent themselves, with no element-count prefix.
                match element {
                    SliceElement::Byte => {
                        let stage = format!("{prefix}field_{}_body", member.number);
                        emitter.enc_varint(
                            &content_label,
                            &ScalarKind::U64.encode_bits_binding(
                                &format!("{prefix}value.{}.len", member.name),
                                prefix,
                            ),
                            &stage,
                        );
                        emitter.enc_slice_bytes(&stage, &member.name, &after);
                    }
                    SliceElement::Scalar(kind) => {
                        emitter.enc_packed_slice(&content_label, &member.name, *kind, &after)
                    }
                }
            }
            MemberShape::FixedVec {
                element, capacity, ..
            } => {
                // The vector's intrinsic `length` bounds the packed
                // elements; the byte length is the only wire framing.
                emitter.enc_packed(
                    &content_label,
                    &format!("{prefix}value.{}.length", member.name),
                    &format!("{prefix}value.{}.items", member.name),
                    Some(&capacity.to_string()),
                    *element,
                    &after,
                );
            }
            MemberShape::NestedMessage { schema } => {
                // `member_facts` already proved the nested schema resolves
                // with scalar-only members.
                let Some((_, sub_members)) = schemas
                    .iter()
                    .find(|(name, _)| name.as_str() == schema.as_str())
                else {
                    continue;
                };
                emitter.enc_nested(&content_label, &member.name, sub_members, &after);
            }
            MemberShape::FixedScalarArray { element, length } => {
                // Packed elements carry a byte-length prefix: measure each
                // element's varint extent, emit that total, then the elements.
                emitter.hop(
                    &content_label,
                    "",
                    &format!("{prefix}field_{}_measure_0", member.number),
                    ", 0",
                );
                for index in 0..*length {
                    let measure_next = if index + 1 == *length {
                        format!("{prefix}field_{}_length", member.number)
                    } else {
                        format!("{prefix}field_{}_measure_{}", member.number, index + 1)
                    };
                    emitter.enc_measure(
                        &format!("{prefix}field_{}_measure_{index}", member.number),
                        &element.encode_bits_binding(
                            &format!("{prefix}value.{}[{index}]", member.name),
                            prefix,
                        ),
                        &measure_next,
                    );
                }
                emitter.machine(
                    &format!("{prefix}field_{}_length", member.number),
                    &format!("{prefix}accumulator: u64 in Wrapping"),
                    "",
                    &format!(
                        "        let {prefix}bits: u64 = {prefix}accumulator as u64;\n        transition {{ _ -> {prefix}field_{}_length_loop({prefix}value, {prefix}out, {prefix}written, {prefix}at, {prefix}bits) }}\n",
                        member.number
                    ),
                );
                let element_zero = format!("{prefix}field_{}_element_0", member.number);
                let length_next: &str = if *length == 0 { &after } else { &element_zero };
                emitter.enc_varint_loop(
                    &format!("{prefix}field_{}_length_loop", member.number),
                    length_next,
                );
                for index in 0..*length {
                    let next = if index + 1 == *length {
                        after.clone()
                    } else {
                        format!("{prefix}field_{}_element_{}", member.number, index + 1)
                    };
                    emitter.enc_varint(
                        &format!("{prefix}field_{}_element_{index}", member.number),
                        &element.encode_bits_binding(
                            &format!("{prefix}value.{}[{index}]", member.name),
                            prefix,
                        ),
                        &next,
                    );
                }
            }
        }
    }
    let declarations = emitter.declarations.clone();
    let _ = writeln!(
        emitter.text,
        "    machine {prefix}encode_finish({declarations}) {{\n        {prefix}written = {prefix}at;\n    }}\n    machine {prefix}encode_overflow({declarations}) {{\n        {prefix}written = {prefix}at;\n    }}\n}}\n"
    );
}

/// `machine S::decode(value: &mut V, buffer: &B, read: &mut u64, verdict:
/// &mut Verdict)`: expect the era bytes, then per field the tag bytes and the
/// content; `read` publishes the cursor and `verdict` the soundness flag.
pub(super) fn emit_decode_machine(
    names: &CodecNames,
    text: &mut String,
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    records: &HashMap<String, RecordShape>,
    schemas: &[(String, Vec<WireMemberShape>)],
    schema_name: &str,
    signature: &[String],
    members: &[WireMemberShape],
) {
    let prefix = names.prefix.as_str();
    let Some(facts_by_member) = member_facts(
        names,
        syntax_trees,
        sources,
        domains,
        records,
        schemas,
        signature_value_type(signature),
        members,
    ) else {
        return;
    };
    let Ok(facts_by_member) = facts_by_member.into_iter().collect::<Result<Vec<_>, _>>() else {
        return;
    };
    // A borrowed slice of non-`u8` elements decodes through owning storage —
    // the zero-copy re-view has nothing to point at — so that member kind
    // keeps the whole machine unemitted rather than dangling a content stage.
    if members.iter().any(|member| {
        matches!(
            member.shape,
            MemberShape::BorrowedSlice {
                element: SliceElement::Scalar(_)
            }
        )
    }) {
        return;
    }
    let verdict_type = signature[3]
        .trim_start_matches('&')
        .trim_start()
        .trim_start_matches("mut")
        .trim_start()
        .to_string();
    let mut emitter = MachineEmitter {
        names,
        text,
        declarations: format!(
            "{prefix}value: {}, {prefix}buffer: {}, {prefix}read: {}, {prefix}verdict: {}, {prefix}at: u64, {prefix}bad: u64 in Wrapping",
            signature[0], signature[1], signature[2], signature[3]
        ),
        call: format!(
            "({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad)"
        ),
        limit: format!("{prefix}buffer.len"),
    };
    let _ = writeln!(
        emitter.text,
        "machine {schema_name}::decode({prefix}value: {}, {prefix}buffer: {}, {prefix}read: {}, {prefix}verdict: {}) {{\n    transition {{ _ -> {prefix}era_0({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, 0, 0) }}\n",
        signature[0], signature[1], signature[2], signature[3]
    );

    let era_bytes = varint_bytes(0);
    let first_tag = members
        .first()
        .map(|member| format!("{prefix}field_{}_tag_0", member.number))
        .unwrap_or_else(|| format!("{prefix}decode_finish"));
    emitter.dec_tag_bytes(
        &era_bytes,
        |index| format!("{prefix}era_{index}"),
        &first_tag,
        "",
        "",
    );

    for (member_index, member) in members.iter().enumerate() {
        let content_label = format!("{prefix}field_{}_content", member.number);
        let after = if member_index + 1 == members.len() {
            format!("{prefix}decode_finish")
        } else {
            format!("{prefix}field_{}_tag_0", members[member_index + 1].number)
        };
        let tag_bytes = varint_bytes(member.number);
        emitter.dec_tag_bytes(
            &tag_bytes,
            |index| format!("{prefix}field_{}_tag_{index}", member.number),
            &content_label,
            "",
            "",
        );
        match &member.shape {
            MemberShape::Scalar(kind) => {
                let store = format!("{prefix}field_{}_store", member.number);
                emitter.dec_varint(&content_label, &store, "", "");
                emitter.dec_scalar_store(
                    &store,
                    &member.name,
                    *kind,
                    &after,
                    &facts_by_member[member_index],
                    "",
                    "",
                );
            }
            MemberShape::FixedVec { element, capacity } => {
                emitter.dec_fixed_vec(
                    &content_label,
                    &member.name,
                    *element,
                    *capacity,
                    &after,
                    &facts_by_member[member_index],
                );
            }
            MemberShape::NestedMessage { schema } => {
                let Some(sub_facts) = nested_sub_facts(
                    names,
                    syntax_trees,
                    sources,
                    domains,
                    records,
                    signature_value_type(signature),
                    &member.name,
                    schemas,
                    schema,
                ) else {
                    continue;
                };
                let Some((_, sub_members)) = schemas
                    .iter()
                    .find(|(name, _)| name.as_str() == schema.as_str())
                else {
                    continue;
                };
                emitter.dec_nested(
                    &content_label,
                    &member.name,
                    sub_members,
                    &sub_facts,
                    &after,
                );
            }
            MemberShape::BorrowedSlice {
                element: SliceElement::Byte,
            } => {
                // `&[u8]` decodes zero-copy: the length varint bounds the
                // content window, and the member re-views `buffer[at..end]`
                // rather than copying the bytes out of the buffer.
                let open = format!("{prefix}field_{}_open", member.number);
                emitter.dec_varint(&content_label, &open, "", "");
                let open_in = format!("{prefix}field_{}_open_in", member.number);
                let view_slack = format!("{prefix}field_{}_slack", member.number);
                let view_prep = format!("{prefix}field_{}_prep", member.number);
                let view_ok = format!("{prefix}field_{}_view", member.number);
                let view_skip = format!("{prefix}field_{}_skip", member.number);
                // `open` guards the cursor bound its own subtraction needs —
                // callers (the shared varint states) cannot carry a premise
                // through, so `at <= buffer.len` is tested here and only the
                // true arm does the range arithmetic.
                emitter.window_open_states(&open, &open_in, &view_slack, &view_prep, &view_skip);
                // The window's far edge derives through its complements, one
                // bound per state: `slack` must be a named local before a
                // guard can establish it (arithmetic call arguments do not
                // substitute into a callee's requires), and each derived
                // bound is re-tested before the state that needs it.
                emitter.machine(
                    &view_prep,
                    &format!("{prefix}slack: u64, {prefix}room: u64"),
                    &format!(
                        "{prefix}slack <= {prefix}room && {prefix}slack <= {} && {prefix}room <= {}",
                        emitter.limit, emitter.limit
                    ),
                    &format!(
                        "        let {prefix}end: u64 = {prefix}buffer.len - {prefix}slack;\n        let {prefix}start: u64 = {prefix}buffer.len - {prefix}room;\n        transition {prefix}start <= {prefix}end && {prefix}end <= {} {{\n            true -> {view_ok}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}start, {prefix}end)\n            _ -> {view_skip}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1, 0)\n        }}\n",
                        emitter.limit
                    ),
                );
                emitter.machine(
                    &view_ok,
                    &format!("{prefix}start: u64, {prefix}end: u64"),
                    &format!("{prefix}start <= {prefix}end && {prefix}end <= {}", emitter.limit),
                    &format!(
                        "        let {prefix}decoded: &[u8] = {prefix}buffer[{prefix}start..{prefix}end];\n        transition {{ _ -> {view_ok}_validate({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}end, {prefix}bad, {prefix}decoded) }}\n"
                    ),
                );
                // Runtime checks establish exactly the view passed to the
                // store. Invalid bytes leave the destination untouched; the
                // codec contract leaves partial output unspecified on Invalid,
                // but ordinary checked fields must retain their invariants.
                let requires = facts_by_member[member_index]
                    .iter()
                    .map(|fact| format!("({})", fact.instantiate(&format!("{prefix}decoded"))))
                    .collect::<Vec<_>>()
                    .join(" && ");
                let write = format!("{view_ok}_write");
                let validation = if requires.is_empty() {
                    format!(
                        "        transition {{ _ -> {write}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}decoded) }}\n"
                    )
                } else {
                    format!(
                        "        transition {requires} {{\n            true -> {write}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad, {prefix}decoded)\n            _ -> {after}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad + 1)\n        }}\n"
                    )
                };
                emitter.machine(
                    &format!("{view_ok}_validate"),
                    &format!("{prefix}decoded: &[u8]"),
                    "",
                    &validation,
                );
                emitter.machine(
                    &write,
                    &format!("{prefix}decoded: &[u8]"),
                    &requires,
                    &format!(
                        "        {prefix}value.{} = {prefix}decoded;\n        transition {{ _ -> {after}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad) }}\n",
                        member.name
                    ),
                );
                emitter.machine(
                    &view_skip,
                    &format!("{prefix}end: u64"),
                    "",
                    &format!(
                        "        transition {{ _ -> {}({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad) }}\n",
                        after
                    ),
                );
            }
            MemberShape::BorrowedSlice { .. } => {}
            MemberShape::FixedScalarArray { element, length } => {
                let close = format!("{content_label}_close");
                let first = if *length == 0 {
                    close.clone()
                } else {
                    format!("{prefix}field_{}_element_0", member.number)
                };
                // Fixed arrays use the same checked byte window as growable
                // collections. The varint decoder cannot promise a cursor
                // bound on malformed input; opening the window establishes it
                // before any subtraction or end calculation.
                emitter.dec_open_window(&content_label, &first);
                for index in 0..*length {
                    let read = format!("{prefix}field_{}_element_{index}", member.number);
                    let store = format!("{prefix}field_{}_element_{index}_store", member.number);
                    emitter.dec_varint(
                        &read,
                        &store,
                        &format!("{prefix}end: u64"),
                        &format!(", {prefix}end"),
                    );
                    let next = if index + 1 == *length {
                        close.clone()
                    } else {
                        format!("{prefix}field_{}_element_{}", member.number, index + 1)
                    };
                    emitter.dec_element_store(
                        &store,
                        &member.name,
                        index,
                        *element,
                        &next,
                        &facts_by_member[member_index],
                    );
                }
                emitter.dec_close_window(&content_label, &after);
            }
        }
    }
    let declarations = emitter.declarations.clone();
    let _ = writeln!(
        emitter.text,
        "    machine {prefix}decode_finish({declarations}) {{\n        {prefix}read = {prefix}at;\n        transition {prefix}bad == 0 {{\n            true -> {prefix}decode_sound({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad)\n            _ -> {prefix}decode_invalid({prefix}value, {prefix}buffer, {prefix}read, {prefix}verdict, {prefix}at, {prefix}bad)\n        }}\n    }}\n    machine {prefix}decode_sound({declarations}) {{\n        {prefix}verdict = {verdict_type}::Sound;\n    }}\n    machine {prefix}decode_invalid({declarations}) {{\n        {prefix}verdict = {verdict_type}::Invalid;\n    }}\n}}\n"
    );
}
