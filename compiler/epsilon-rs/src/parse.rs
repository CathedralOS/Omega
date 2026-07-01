// Parser: tokens -> AST. Platform-independent. Enforces the Alpha subset (an
// unsupported construct is a hard error, not a silent acceptance).
//
// Grammar handled so far:
//   program := item*
//   item    := ("boundary" | "data") <balanced braces, skipped>
//            | "machine" <header> "{" stmt* "}"
//   stmt    := "let" IDENT ":" TYPE "=" expr ";"?
//            | path "(" expr ")" ";"?            (path's last segment = exit_process)
//   expr    := add ; add := mul (("+"|"-") mul)* ; mul := primary (("*"|"/") primary)*
//   primary := INT | IDENT(local) | "(" expr ")"

use crate::ast::{BinaryOp, Expr, Machine, Pattern, Program, Statement, TransitionArm};
use crate::lex::{token_text, Token, TokenKind};

// How a data field is laid out / what it carries.
#[derive(Clone, Copy)]
enum FieldKind {
    Scalar,      // i32-width value, occupies 8 bytes
    Boundary,    // a capability handle (e.g. Console): zero runtime storage
    Data(usize),     // a nested data type (occupies that type's size)
    Array(i32, i32), // [scalar; N]: (count, element_bytes); u8 = 1 byte, others = 8
}

struct DataFieldInfo {
    name: Vec<u8>,
    offset: i32, // byte offset within the struct
    kind: FieldKind,
    enum_index: Option<usize>, // Some(idx) if this scalar slot holds a tag-only enum's tag
    domain: u8,                // arithmetic domain of a scalar field: 0 Trapping, 1 Wrapping, 2 Saturating
    range: Option<(i32, i32)>, // range refinement `i32 in lo..hi`: every store owes value in [lo, hi)
}

// The field's domain, if `self.<field>` names a scalar field (else 0 = default Trapping).
impl Parser<'_> {
    fn self_field_domain(&self, field: &[u8]) -> u8 {
        if let Some(data_type) = self.self_data_type {
            if let Some(info) = self.data_field_maps[data_type].iter().find(|f| f.name.as_slice() == field) {
                return info.domain;
            }
        }
        0
    }
    // The range refinement of `self.<field>`, if it has one (`i32 in lo..hi`): every store owes value in [lo,hi).
    fn self_field_range(&self, field: &[u8]) -> Option<(i32, i32)> {
        let data_type = self.self_data_type?;
        self.data_field_maps[data_type]
            .iter()
            .find(|f| f.name.as_slice() == field)
            .and_then(|info| info.range)
    }
}

fn is_scalar_type(type_name: &[u8]) -> bool {
    matches!(
        type_name,
        b"i8" | b"u8" | b"i16" | b"u16" | b"i32" | b"u32" | b"i64" | b"u64" | b"usize" | b"isize"
            | b"bool"
    )
}

pub struct Parser<'a> {
    tokens: &'a [Token],
    source: &'a [u8],
    position: usize,
    expressions: Vec<Expr>,
    call_args: Vec<usize>,
    strings: Vec<Vec<u8>>,
    machines: Vec<Machine>,
    machine_names: Vec<Vec<u8>>,      // pre-scanned callable names, in machine-index order
    machine_self_types: Vec<Option<usize>>, // per machine: its receiver data type, if any
    boundary_names: Vec<Vec<u8>>,     // declared `boundary trait` names (zero-size fields)
    data_type_names: Vec<Vec<u8>>,    // declared `data` type names
    data_type_sizes: Vec<i32>,        // byte size of each data type
    data_field_maps: Vec<Vec<DataFieldInfo>>, // fields of each data type
    enum_names: Vec<Vec<u8>>,         // declared enum (`data X { case ... }`) names
    enum_variant_lists: Vec<Vec<Vec<u8>>>, // per enum: variant names; the tag is the index
    enum_variant_payload_count: Vec<Vec<usize>>, // per enum, per variant: number of i32 payload fields
    current_arm_bindings: Vec<(Vec<u8>, usize)>, // `Variant { a, b }` payload bindings -> synth payload-read nodes
    state_param_names: Vec<Vec<Vec<u8>>>, // per state (parallel to state_names): its param names
    current_state_params: Vec<Vec<u8>>, // params of the state body being parsed (resolve to arg slots)
    state_arg_base: usize,            // local index of the shared state-arg slot 0 in this machine
    local_names: Vec<Vec<u8>>,        // locals of the machine being parsed (params first)
    state_names: Vec<Vec<u8>>,        // pre-scanned state names of the machine being parsed
    self_data_type: Option<usize>,    // data type of `self` in the machine being parsed
    current_machine_makes_call: bool, // set while parsing a machine that calls out
    uses_host_io: bool,               // any machine uses a host op => program needs the import table
    // Bounded-STORE obligations for the machine being parsed: each `self.<range field> = value` records
    // (value node, lo, hi) so static discharge can prove `value in [lo, hi)`. See discharge_field_stores.
    store_obligations: Vec<(usize, i32, i32)>,
    // Candidate range-typed lets: each `let x: i32 in lo..hi = init` records (local index, init node). If x is
    // never reassigned it equals `init` throughout, so downstream `self.arr[x]` discharges through `init`.
    local_range_candidates: Vec<(usize, usize)>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token], source: &'a [u8]) -> Parser<'a> {
        Parser {
            tokens,
            source,
            position: 0,
            expressions: Vec::new(),
            call_args: Vec::new(),
            strings: Vec::new(),
            machines: Vec::new(),
            machine_names: Vec::new(),
            machine_self_types: Vec::new(),
            boundary_names: Vec::new(),
            data_type_names: Vec::new(),
            data_type_sizes: Vec::new(),
            data_field_maps: Vec::new(),
            enum_names: Vec::new(),
            enum_variant_lists: Vec::new(),
            enum_variant_payload_count: Vec::new(),
            current_arm_bindings: Vec::new(),
            state_param_names: Vec::new(),
            current_state_params: Vec::new(),
            state_arg_base: 0,
            local_names: Vec::new(),
            state_names: Vec::new(),
            self_data_type: None,
            current_machine_makes_call: false,
            uses_host_io: false,
            store_obligations: Vec::new(),
            local_range_candidates: Vec::new(),
        }
    }

    fn find_state_index(&self, name: &[u8]) -> Option<usize> {
        self.state_names.iter().position(|state_name| state_name.as_slice() == name)
    }

    fn find_machine_index(&self, name: &[u8]) -> Option<usize> {
        self.machine_names.iter().position(|machine_name| machine_name.as_slice() == name)
    }

    fn find_data_type(&self, name: &[u8]) -> Option<usize> {
        self.data_type_names.iter().position(|type_name| type_name.as_slice() == name)
    }

    // Resolve `self.<field>` to its byte offset; only scalar fields can be read or
    // written directly (boundary/nested-data field access is a later slice).
    fn self_field_offset(&self, field: &[u8]) -> Result<i32, String> {
        let data_type = self.self_data_type.ok_or_else(|| {
            "alpha-onramp: `self` used in a machine with no receiver".to_string()
        })?;
        let info = self.data_field_maps[data_type]
            .iter()
            .find(|field_info| field_info.name.as_slice() == field)
            .ok_or_else(|| {
                format!("alpha-onramp: unknown field 'self.{}'", String::from_utf8_lossy(field))
            })?;
        match info.kind {
            FieldKind::Scalar => Ok(info.offset),
            FieldKind::Boundary => Err(format!(
                "alpha-onramp: 'self.{}' is a capability, not a readable value",
                String::from_utf8_lossy(field)
            )),
            FieldKind::Data(_) => Err(format!(
                "alpha-onramp: 'self.{}' is a nested struct; only scalar fields are supported (slice 7a)",
                String::from_utf8_lossy(field)
            )),
            FieldKind::Array(..) => Err(format!(
                "alpha-onramp: 'self.{}' is an array; index it with [..]",
                String::from_utf8_lossy(field)
            )),
        }
    }

    // Resolve a nested `self.a.b.c…` path to (total byte offset, final field's arithmetic domain).
    // Nested data is inlined, so offsets just sum; every segment but the last must be a nested-data
    // field, and the last must be scalar. `self.a` (single segment) still goes through self_field_offset.
    fn self_field_path(&self, path: &[Vec<u8>]) -> Result<(i32, u8), String> {
        let mut data_type = self.self_data_type.ok_or_else(|| {
            "alpha-onramp: `self` used in a machine with no receiver".to_string()
        })?;
        let mut offset = 0i32;
        for (i, seg) in path.iter().enumerate() {
            let info = self.data_field_maps[data_type]
                .iter()
                .find(|field_info| field_info.name.as_slice() == seg.as_slice())
                .ok_or_else(|| {
                    format!("alpha-onramp: unknown field '{}' in a self.* path", String::from_utf8_lossy(seg))
                })?;
            let last = i == path.len() - 1;
            match info.kind {
                FieldKind::Data(idx) => {
                    if last {
                        return Err(format!(
                            "alpha-onramp: 'self…{}' is a nested struct, not a scalar value",
                            String::from_utf8_lossy(seg)
                        ));
                    }
                    offset += info.offset;
                    data_type = idx;
                }
                FieldKind::Scalar => {
                    if !last {
                        return Err(format!(
                            "alpha-onramp: 'self…{}' is scalar; cannot select a field of it",
                            String::from_utf8_lossy(seg)
                        ));
                    }
                    return Ok((offset + info.offset, info.domain));
                }
                _ => {
                    return Err(format!(
                        "alpha-onramp: 'self…{}' must be a nested-data or scalar field",
                        String::from_utf8_lossy(seg)
                    ));
                }
            }
        }
        Err("alpha-onramp: empty self field path".to_string())
    }

    // Resolve `self.<array>` to (byte offset, element count, element bytes) for indexing.
    fn self_array_field(&self, field: &[u8]) -> Result<(i32, i32, i32), String> {
        let data_type = self.self_data_type.ok_or_else(|| {
            "alpha-onramp: `self` used in a machine with no receiver".to_string()
        })?;
        let info = self.data_field_maps[data_type]
            .iter()
            .find(|field_info| field_info.name.as_slice() == field)
            .ok_or_else(|| {
                format!("alpha-onramp: unknown field 'self.{}'", String::from_utf8_lossy(field))
            })?;
        match info.kind {
            FieldKind::Array(count, element_bytes) => Ok((info.offset, count, element_bytes)),
            _ => Err(format!(
                "alpha-onramp: 'self.{}' is not an array",
                String::from_utf8_lossy(field)
            )),
        }
    }

    fn current_kind(&self) -> TokenKind {
        self.tokens[self.position].kind
    }
    fn current_text(&self) -> &[u8] {
        token_text(&self.tokens[self.position], self.source)
    }
    fn bump(&mut self) -> Token {
        let token = self.tokens[self.position];
        self.position += 1;
        token
    }
    fn expect(&mut self, kind: TokenKind) -> Result<Token, String> {
        if self.current_kind() == kind {
            Ok(self.bump())
        } else {
            Err(format!(
                "alpha-onramp: parse error: expected {:?}, found {:?}",
                kind,
                self.current_kind()
            ))
        }
    }
    fn add_expression(&mut self, expression: Expr) -> usize {
        self.expressions.push(expression);
        self.expressions.len() - 1
    }
    // Read an integer-literal token's value (for a range-refinement bound `lo..hi`).
    fn parse_int_token(&mut self) -> Result<i32, String> {
        let token = self.expect(TokenKind::Int)?;
        let text = std::str::from_utf8(token_text(&token, self.source)).unwrap();
        let value: i64 = text
            .parse()
            .map_err(|_| format!("alpha-onramp: bad integer in range type '{}'", text))?;
        if value > i32::MAX as i64 {
            return Err(format!("alpha-onramp: range bound {} out of i32 range", value));
        }
        Ok(value as i32)
    }
    fn find_local_index(&self, name: &[u8]) -> Option<usize> {
        // a state's parameters resolve to the machine's shared state-arg slots (by position)
        if let Some(position) = self.current_state_params.iter().position(|p| p.as_slice() == name) {
            return Some(self.state_arg_base + position);
        }
        self.local_names.iter().position(|local_name| local_name.as_slice() == name)
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        self.prescan_machine_names()?; // so a call can name a machine defined later
        while self.current_kind() != TokenKind::Eof {
            if self.current_kind() != TokenKind::Ident {
                return Err(format!(
                    "alpha-onramp: parse error: expected a top-level item, found {:?}",
                    self.current_kind()
                ));
            }
            match self.current_text() {
                b"boundary" => self.parse_boundary()?,
                b"data" => self.parse_data()?,
                b"machine" => {
                    let machine = self.parse_machine()?;
                    self.machines.push(machine);
                }
                other => {
                    return Err(format!(
                        "alpha-onramp: unsupported top-level keyword '{}' (Alpha subset)",
                        String::from_utf8_lossy(other)
                    ))
                }
            }
        }
        if self.machines.is_empty() {
            return Err("alpha-onramp: no machine to compile".into());
        }
        let entry_machine = self
            .find_machine_index(b"main")
            .ok_or_else(|| "alpha-onramp: no entry machine 'main'".to_string())?;
        let entry_data_size = match self.machine_self_types[entry_machine] {
            Some(data_type) => self.data_type_sizes[data_type],
            None => 0,
        };
        Ok(Program {
            machines: std::mem::take(&mut self.machines),
            entry_machine,
            entry_data_size,
            uses_imports: self.uses_host_io,
            expressions: std::mem::take(&mut self.expressions),
            call_args: std::mem::take(&mut self.call_args),
            strings: std::mem::take(&mut self.strings),
        })
    }

    // `boundary trait NAME { ... }` — record NAME (a capability type = zero-size
    // field), then skip the body (its methods are declarations the on-ramp hardwires).
    fn parse_boundary(&mut self) -> Result<(), String> {
        let mut probe = self.position;
        let mut name: Option<Vec<u8>> = None;
        while self.tokens[probe].kind != TokenKind::LBrace {
            if self.tokens[probe].kind == TokenKind::Eof {
                return Err("alpha-onramp: parse error: boundary without a body".into());
            }
            if self.tokens[probe].kind == TokenKind::Ident {
                name = Some(token_text(&self.tokens[probe], self.source).to_vec());
            }
            probe += 1;
        }
        if let Some(name) = name {
            self.boundary_names.push(name);
        }
        self.skip_braced_item()
    }

    // `data NAME { field: Type; ... }` — lay out fields (scalar = 8 bytes, boundary
    // = 0, nested data = its size) and record the type's name, size, and field map.
    fn parse_data(&mut self) -> Result<(), String> {
        self.bump(); // data
        let name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut variants: Vec<Vec<u8>> = Vec::new();
        let mut variant_payloads: Vec<usize> = Vec::new();
        let mut offset = 0i32;
        while self.current_kind() != TokenKind::RBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated data body".into());
            }
            // enum variant: `case Name;` (tag-only) or `case Name(field: i32);` (one payload,
            // stored at the enum's offset + 8; the field name is bound at the match site).
            if self.current_kind() == TokenKind::Ident
                && token_text(&self.tokens[self.position], self.source) == b"case"
            {
                self.bump(); // case
                let variant = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                let mut payload_count = 0usize;
                if self.current_kind() == TokenKind::LParen {
                    self.bump(); // (
                    while self.current_kind() != TokenKind::RParen {
                        if self.current_kind() == TokenKind::Eof {
                            return Err("alpha-onramp: parse error: unterminated payload field list".into());
                        }
                        self.expect(TokenKind::Ident)?; // field name (bound at the match site)
                        self.expect(TokenKind::Colon)?;
                        self.expect(TokenKind::Ident)?; // type (everything is i32)
                        payload_count += 1;
                        if self.current_kind() == TokenKind::Comma {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                }
                variants.push(variant);
                variant_payloads.push(payload_count);
                if self.current_kind() == TokenKind::Semi || self.current_kind() == TokenKind::Comma {
                    self.bump();
                }
                continue;
            }
            let field_name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
            self.expect(TokenKind::Colon)?;
            let (kind, size, enum_index) = if self.current_kind() == TokenKind::LBracket {
                // `[ElementType; N]` — N scalar elements of 8 bytes each
                self.bump(); // [
                let element = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                if !is_scalar_type(&element) {
                    return Err("alpha-onramp: array elements must be scalar (slice 7b)".into());
                }
                self.expect(TokenKind::Semi)?;
                let count_token = self.expect(TokenKind::Int)?;
                let count: i32 = std::str::from_utf8(token_text(&count_token, self.source))
                    .unwrap()
                    .parse()
                    .map_err(|_| "alpha-onramp: bad array length".to_string())?;
                self.expect(TokenKind::RBracket)?;
                let element_bytes = if element == b"u8" { 1 } else { 8 };
                (FieldKind::Array(count, element_bytes), count * element_bytes, None)
            } else if self.current_kind() == TokenKind::Amp {
                return Err("alpha-onramp: ref/slice data fields are unsupported (slice 7a)".into());
            } else {
                let type_name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                let kind = self.classify_field_type(&type_name)?;
                let enum_index = self.find_enum(&type_name);
                let size = match enum_index {
                    // an enum field reserves a tag word (+ a payload word if the enum has payloads)
                    Some(idx) => self.enum_size(idx),
                    None => match kind {
                        FieldKind::Scalar => 8,
                        FieldKind::Boundary => 0,
                        FieldKind::Data(index) => self.data_type_sizes[index],
                        FieldKind::Array(count, element_bytes) => count * element_bytes,
                    },
                };
                (kind, size, enum_index)
            };
            // optional `in <refinement>` on a scalar field. Two forms, distinguished by the token after `in`:
            //   `in Wrapping`/`in Saturating` (Ident) -> arithmetic domain (changes codegen);
            //   `in lo..hi`      (Int)   -> RANGE refinement (Omega's bounded type): every store to this field
            //                               owes `value in [lo, hi)`, discharged statically like an array bound.
            let mut domain: u8 = 0;
            let mut range: Option<(i32, i32)> = None;
            if self.current_kind() == TokenKind::Ident && self.current_text() == b"in" {
                self.bump(); // in
                if self.current_kind() == TokenKind::Int {
                    let lo = self.parse_int_token()?;
                    self.expect(TokenKind::Dot)?;
                    self.expect(TokenKind::Dot)?; // ..
                    let hi = self.parse_int_token()?;
                    range = Some((lo, hi));
                } else if self.current_kind() == TokenKind::Ident {
                    if self.current_text() == b"Wrapping" {
                        domain = 1;
                        self.bump();
                    } else if self.current_text() == b"Saturating" {
                        domain = 2;
                        self.bump();
                    }
                }
            }
            fields.push(DataFieldInfo { name: field_name, offset, kind, enum_index, domain, range });
            offset += size;
            // skip any remaining type tokens (e.g. `in Utf8`) up to the separator
            while self.current_kind() != TokenKind::Semi
                && self.current_kind() != TokenKind::Comma
                && self.current_kind() != TokenKind::RBrace
            {
                if self.current_kind() == TokenKind::Eof {
                    return Err("alpha-onramp: parse error: unterminated data field".into());
                }
                self.bump();
            }
            if self.current_kind() == TokenKind::Semi || self.current_kind() == TokenKind::Comma {
                self.bump();
            }
        }
        self.bump(); // '}'
        if !variants.is_empty() {
            // An enum, not a struct: its value is a tag word (the variant index), optionally
            // followed by a single i32 payload word. Stored/matched via the tag; kept out of
            // the data-struct tables and in the enum tables.
            if !fields.is_empty() {
                return Err("alpha-onramp: mixing fields and `case` variants is not yet supported".into());
            }
            self.enum_names.push(name);
            self.enum_variant_lists.push(variants);
            self.enum_variant_payload_count.push(variant_payloads);
            return Ok(());
        }
        self.data_type_names.push(name);
        self.data_type_sizes.push(offset);
        self.data_field_maps.push(fields);
        Ok(())
    }

    fn find_enum(&self, name: &[u8]) -> Option<usize> {
        self.enum_names.iter().position(|enum_name| enum_name.as_slice() == name)
    }

    // byte size of an enum value: a tag word, plus one word per payload field of the
    // widest variant (so every variant's payload fits at offset + 8, + 16, ...).
    fn enum_size(&self, enum_index: usize) -> i32 {
        let max = self.enum_variant_payload_count[enum_index]
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        8 + (max as i32) * 8
    }

    // If the assignment RHS is `Enum::Variant(arg)`, parse it and desugar to a Block that
    // stores the tag at the field and the payload at field+8. Returns None (consuming nothing)
    // if the RHS is not an enum-payload construction, so the normal expression path runs.
    fn try_parse_enum_construction(&mut self, field: &[u8]) -> Result<Option<Statement>, String> {
        if self.current_kind() != TokenKind::Ident {
            return Ok(None);
        }
        let name = token_text(&self.tokens[self.position], self.source).to_vec();
        let enum_index = match self.find_enum(&name) {
            Some(index) => index,
            None => return Ok(None),
        };
        // require the `Enum :: Variant (` shape; a tag-only `Enum::Variant` falls through to
        // the expression path (which lowers it to the tag and stores it as a scalar).
        if self.tokens[self.position + 1].kind != TokenKind::ColonColon
            || self.tokens[self.position + 2].kind != TokenKind::Ident
            || self.tokens[self.position + 3].kind != TokenKind::LParen
        {
            return Ok(None);
        }
        self.bump(); // Enum
        self.bump(); // ::
        let variant = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
        let tag = self.enum_variant_tag(enum_index, &variant).ok_or_else(|| {
            format!(
                "alpha-onramp: '{}' is not a variant of enum '{}'",
                String::from_utf8_lossy(&variant),
                String::from_utf8_lossy(&name)
            )
        })?;
        let count = self.enum_variant_payload_count[enum_index][tag as usize];
        if count == 0 {
            return Err(format!(
                "alpha-onramp: variant '{}' has no payload to construct",
                String::from_utf8_lossy(&variant)
            ));
        }
        self.bump(); // (
        let mut args = Vec::new();
        while self.current_kind() != TokenKind::RParen {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated enum construction".into());
            }
            args.push(self.parse_expression()?);
            if self.current_kind() == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        if args.len() != count {
            return Err(format!(
                "alpha-onramp: variant '{}' takes {} payload value(s), got {}",
                String::from_utf8_lossy(&variant),
                count,
                args.len()
            ));
        }
        let field_offset = self.self_field_offset(field)?;
        let tag_node = self.add_expression(Expr::Int(tag));
        // tag at field+0, payloads at field+8, +16, ...
        let mut stmts = vec![Statement::StoreSelfField(field_offset, tag_node, 0)];
        for (index, arg) in args.into_iter().enumerate() {
            stmts.push(Statement::StoreSelfField(field_offset + 8 + (index as i32) * 8, arg, 0));
        }
        Ok(Some(Statement::Block(stmts)))
    }

    fn enum_variant_tag(&self, enum_index: usize, variant: &[u8]) -> Option<i32> {
        self.enum_variant_lists[enum_index]
            .iter()
            .position(|candidate| candidate.as_slice() == variant)
            .map(|index| index as i32)
    }

    fn classify_field_type(&self, type_name: &[u8]) -> Result<FieldKind, String> {
        if self.boundary_names.iter().any(|boundary| boundary.as_slice() == type_name) {
            return Ok(FieldKind::Boundary);
        }
        if let Some(index) = self.find_data_type(type_name) {
            return Ok(FieldKind::Data(index));
        }
        if self.find_enum(type_name).is_some() {
            // a tag-only enum field is just an 8-byte slot holding the i32 tag
            return Ok(FieldKind::Scalar);
        }
        if is_scalar_type(type_name) {
            return Ok(FieldKind::Scalar);
        }
        Err(format!(
            "alpha-onramp: unknown field type '{}'",
            String::from_utf8_lossy(type_name)
        ))
    }

    // Record each top-level machine's callable name (the identifier just before its
    // parameter-list '('), so a call can refer to a machine declared later. Only
    // depth-0 `machine` tokens count — `machine` inside a `boundary`/`data` block is
    // a boundary method declaration, not a top-level machine.
    fn prescan_machine_names(&mut self) -> Result<(), String> {
        let mut scan = 0usize;
        let mut depth = 0i32;
        while self.tokens[scan].kind != TokenKind::Eof {
            match self.tokens[scan].kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth -= 1,
                TokenKind::Ident
                    if depth == 0 && token_text(&self.tokens[scan], self.source) == b"machine" =>
                {
                    let mut last_ident: Option<usize> = None;
                    let mut probe = scan + 1;
                    while self.tokens[probe].kind != TokenKind::LParen
                        && self.tokens[probe].kind != TokenKind::Eof
                    {
                        if self.tokens[probe].kind == TokenKind::Ident {
                            last_ident = Some(probe);
                        }
                        probe += 1;
                    }
                    match last_ident {
                        Some(index) => self
                            .machine_names
                            .push(token_text(&self.tokens[index], self.source).to_vec()),
                        None => return Err("alpha-onramp: parse error: machine without a name".into()),
                    }
                }
                _ => {}
            }
            scan += 1;
        }
        Ok(())
    }

    // machine := "machine" path "(" params ")" ("->" type)? "{" body "}"
    // path    := IDENT ("::" IDENT)*   (callable name = last segment)
    fn parse_machine(&mut self) -> Result<Machine, String> {
        self.local_names.clear();
        self.state_names.clear();
        self.state_param_names.clear();
        self.current_state_params.clear();
        self.state_arg_base = 0;
        self.self_data_type = None;
        self.current_machine_makes_call = false;
        self.store_obligations = Vec::new();
        self.local_range_candidates = Vec::new();
        self.bump(); // "machine"

        // name path: the first segment is the receiver type for a method (`Foo::m`);
        // resolution + entry lookup use the pre-scan, so we only need the first here.
        let receiver_type = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
        while self.current_kind() == TokenKind::ColonColon {
            self.bump();
            self.expect(TokenKind::Ident)?;
        }

        self.expect(TokenKind::LParen)?;
        let mut param_count = 0usize;
        let mut has_self = false;
        // Range-refinement param types (`i: i32 in lo..hi`): collected here, desugared into preconditions
        // (lo <= i, i < hi) once the precondition list exists. Omega's bounded type -- the type IS the contract.
        let mut param_ranges: Vec<(usize, i32, i32)> = Vec::new();
        while self.current_kind() != TokenKind::RParen {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated parameter list".into());
            }
            if self.current_kind() == TokenKind::Amp {
                // receiver `&mut self` / `&self` — the machine's data pointer, not a value param
                self.bump(); // &
                if self.current_kind() == TokenKind::Ident && self.current_text() == b"mut" {
                    self.bump();
                }
                self.expect(TokenKind::Ident)?; // self
                has_self = true;
            } else if self.current_kind() == TokenKind::Ident && self.current_text() == b"self" {
                self.bump(); // by-value self
                has_self = true;
            } else {
                // value parameter: name ":" type  (the first params become locals 0..n)
                let param_name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                self.expect(TokenKind::Colon)?;
                while self.current_kind() != TokenKind::Comma
                    && self.current_kind() != TokenKind::RParen
                    && !(self.current_kind() == TokenKind::Ident && self.current_text() == b"in")
                {
                    if self.current_kind() == TokenKind::Eof {
                        return Err("alpha-onramp: parse error: unterminated parameter type".into());
                    }
                    self.bump(); // skip the type (slice 6: everything is i32-width)
                }
                let this_param = param_count; // this parameter's local index (params are locals 0..n)
                // RANGE REFINEMENT `i32 in lo..hi` (Omega's bounded type). Desugar into two preconditions on
                // this parameter, so the type IS the contract: an access self.arr[i] with i: i32 in 0..len
                // discharges its bounds obligation from the type (no explicit `requires i < len` needed).
                if self.current_kind() == TokenKind::Ident && self.current_text() == b"in" {
                    self.bump(); // in
                    let lo = self.parse_int_token()?;
                    self.expect(TokenKind::Dot)?;
                    self.expect(TokenKind::Dot)?; // ..
                    let hi = self.parse_int_token()?;
                    param_ranges.push((this_param, lo, hi));
                }
                self.local_names.push(param_name);
                param_count += 1;
            }
            if self.current_kind() == TokenKind::Comma {
                self.bump();
            }
        }
        self.expect(TokenKind::RParen)?;

        // RANGE-REFINED RETURN `-> i32 in lo..hi` (Omega's BoundedStateReturn / output contract): every
        // `return e` owes `e in [lo, hi)`, discharged statically. Captured here, turned into obligations once
        // the return-value nodes are collected below.
        let mut return_range: Option<(i32, i32)> = None;
        if self.current_kind() == TokenKind::Arrow {
            self.bump();
            while self.current_kind() != TokenKind::LBrace
                && !(self.current_kind() == TokenKind::Ident
                    && (self.current_text() == b"requires"
                        || self.current_text() == b"ensures"
                        || self.current_text() == b"in"))
            {
                if self.current_kind() == TokenKind::Eof {
                    return Err("alpha-onramp: parse error: unterminated return type".into());
                }
                self.bump(); // skip the return type
            }
            if self.current_kind() == TokenKind::Ident && self.current_text() == b"in" {
                self.bump(); // in
                if self.current_kind() == TokenKind::Int {
                    let lo = self.parse_int_token()?;
                    self.expect(TokenKind::Dot)?;
                    self.expect(TokenKind::Dot)?; // ..
                    let hi = self.parse_int_token()?;
                    return_range = Some((lo, hi));
                }
            }
        }

        // Optional `requires`/`ensures` contracts — Omega's contract syntax, in any order. A
        // `requires <cond>` is checked at machine entry; an `ensures <cond>` (which may name the
        // special local `result`, the returned value) is checked at every return site. Both
        // desugar to asserts: a false condition TRAPS, exactly like an inline `assert`. This is
        // the declarative, dynamic half of contracts — the obligation lives on the signature.
        // (The static, proof-carrying half is the convergence's certify-*.) Preconditions range
        // over the parameters (locals 0..param_count); postconditions also over `result`.
        let mut preconditions = Vec::new();
        // Desugar range-refinement param types into preconditions: `i: i32 in lo..hi` -> `i >= lo` and `i < hi`.
        // They become entry asserts (checked at runtime) AND are citable by the static discharge (array-bounds,
        // call args) exactly like a hand-written `requires` -- so a bounded type gives memory safety for free.
        for &(param_index, lo, hi) in &param_ranges {
            let local = self.add_expression(Expr::Local(param_index));
            let lo_node = self.add_expression(Expr::Int(lo));
            preconditions.push(self.add_expression(Expr::Binary(BinaryOp::Ge, local, lo_node)));
            let local2 = self.add_expression(Expr::Local(param_index));
            let hi_node = self.add_expression(Expr::Int(hi));
            preconditions.push(self.add_expression(Expr::Binary(BinaryOp::Lt, local2, hi_node)));
        }
        let mut postconditions = Vec::new();
        let mut result_index = 0usize;
        loop {
            if self.current_kind() == TokenKind::Ident && self.current_text() == b"requires" {
                self.bump();
                preconditions.push(self.parse_expression()?);
            } else if self.current_kind() == TokenKind::Ident && self.current_text() == b"ensures" {
                if postconditions.is_empty() {
                    // reserve `result` (the returned value) as a local the postcondition can name
                    result_index = self.local_names.len();
                    self.local_names.push(b"result".to_vec());
                }
                self.bump();
                postconditions.push(self.parse_expression()?);
            } else {
                break;
            }
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
        }

        // `self`'s data type is the receiver (the name path's first segment), if any
        let self_data_type = if has_self { self.find_data_type(&receiver_type) } else { None };
        self.self_data_type = self_data_type;
        self.machine_self_types.push(self_data_type);

        let (mut entry, mut states) = self.parse_machine_body()?;
        // Capture the body's return-value nodes BEFORE desugaring, so static discharge can
        // form the obligation (postcondition with `result` := the returned expression).
        let mut return_exprs = Vec::new();
        Self::collect_return_exprs(&entry, &mut return_exprs);
        for block in &states {
            Self::collect_return_exprs(block, &mut return_exprs);
        }
        // A range-refined return type `-> i32 in lo..hi` turns each returned value into a bounded-value
        // obligation `value in [lo, hi)` -- the output contract, discharged by the same machinery as a
        // bounded store (literal -> ground, range-typed param -> weaken). Reuses `store_obligations`.
        if let Some((lo, hi)) = return_range {
            for &ret in &return_exprs {
                self.store_obligations.push((ret, lo, hi));
            }
        }
        // A range let's declared range is a sound downstream fact only if the local is never reassigned
        // (an `x = ..` would let x escape its type). Keep only the never-reassigned candidates.
        let mut reassigned = std::collections::HashSet::new();
        Self::collect_assigned_locals(&entry, &mut reassigned);
        for block in &states {
            Self::collect_assigned_locals(block, &mut reassigned);
        }
        let local_inits: Vec<(usize, usize)> = self
            .local_range_candidates
            .drain(..)
            .filter(|(idx, _)| !reassigned.contains(idx))
            .collect();
        // Retain the preconditions for static call-site discharge before they are consumed.
        let preconditions_kept = preconditions.clone();
        // Prepend the precondition checks (in source order) so they run before the body.
        for cond in preconditions.into_iter().rev() {
            entry.insert(0, Statement::Assert(cond));
        }
        // Postconditions: check `ensures` at every return site, with `result` bound to the
        // value being returned. Rewrite `return e` -> { result = e; assert <cond>...; return result }.
        let result_local = if postconditions.is_empty() { None } else { Some(result_index) };
        if !postconditions.is_empty() {
            entry = self.rewrite_returns_with_postconditions(entry, result_index, &postconditions);
            states = states
                .into_iter()
                .map(|block| self.rewrite_returns_with_postconditions(block, result_index, &postconditions))
                .collect();
        }
        Ok(Machine {
            result_local,
            postconditions,
            return_exprs,
            preconditions: preconditions_kept,
            param_count,
            local_count: self.local_names.len(),
            makes_call: self.current_machine_makes_call,
            has_self,
            entry,
            states,
            store_obligations: std::mem::take(&mut self.store_obligations),
            local_inits,
        })
    }

    // Collect the indices of all locals that are ASSIGNED (`x = ..`) anywhere in a statement list, recursing
    // into Blocks. A range let whose local appears here can't have its range trusted downstream.
    fn collect_assigned_locals(statements: &[Statement], out: &mut std::collections::HashSet<usize>) {
        for statement in statements {
            match statement {
                Statement::Assign(index, _) => {
                    out.insert(*index);
                }
                Statement::Block(inner) => Self::collect_assigned_locals(inner, out),
                _ => {}
            }
        }
    }

    // Collect the value-expression node of every `return` in a statement list (recursing
    // into Blocks), for static contract discharge. Read-only; runs before desugaring.
    fn collect_return_exprs(statements: &[Statement], out: &mut Vec<usize>) {
        for statement in statements {
            match statement {
                Statement::Return(value) => out.push(*value),
                Statement::Block(inner) => Self::collect_return_exprs(inner, out),
                _ => {}
            }
        }
    }

    // Rewrite each `return e` into `{ result = e; assert <postcondition>...; return result }`,
    // so a machine's `ensures` clauses are checked on every value it yields. Recurses into
    // Blocks; transitions carry no nested returns. Shares the postcondition expr nodes across
    // sites (read-only at lowering) and mints a fresh `result` read per return.
    fn rewrite_returns_with_postconditions(
        &mut self,
        statements: Vec<Statement>,
        result_index: usize,
        postconditions: &[usize],
    ) -> Vec<Statement> {
        let mut out = Vec::with_capacity(statements.len());
        for statement in statements {
            match statement {
                Statement::Return(value) => {
                    let mut block = Vec::new();
                    block.push(Statement::Assign(result_index, value));
                    for &cond in postconditions {
                        block.push(Statement::Assert(cond));
                    }
                    let result_node = self.add_expression(Expr::Local(result_index));
                    block.push(Statement::Return(result_node));
                    out.push(Statement::Block(block));
                }
                Statement::Block(inner) => {
                    let rewritten =
                        self.rewrite_returns_with_postconditions(inner, result_index, postconditions);
                    out.push(Statement::Block(rewritten));
                }
                other => out.push(other),
            }
        }
        out
    }

    fn skip_braced_item(&mut self) -> Result<(), String> {
        while self.current_kind() != TokenKind::LBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: expected '{' in item".into());
            }
            self.bump();
        }
        let mut depth = 0i32;
        loop {
            match self.current_kind() {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth -= 1;
                    self.bump();
                    if depth == 0 {
                        return Ok(());
                    }
                    continue;
                }
                TokenKind::Eof => return Err("alpha-onramp: parse error: unbalanced braces".into()),
                _ => {}
            }
            self.bump();
        }
    }

    // Parse the `{ entry-stmts state-decls }` body; the header (name, params,
    // return type) has already been consumed by parse_machine, and local_names /
    // state_names have been reset there (params are already in local_names).
    fn parse_machine_body(&mut self) -> Result<(Vec<Statement>, Vec<Vec<Statement>>), String> {
        while self.current_kind() != TokenKind::LBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: expected machine body '{'".into());
            }
            self.bump();
        }
        self.bump(); // '{'
        self.prescan_state_names()?; // so transitions can name states declared later

        // Reserve the machine's shared state-arg slots right after its params (before any
        // body locals), so a forward-referencing transition and the state body agree on
        // them. Machines with no state parameters reserve none — their frame is unchanged.
        let max_state_params = self.state_param_names.iter().map(|p| p.len()).max().unwrap_or(0);
        self.state_arg_base = self.local_names.len();
        for slot in 0..max_state_params {
            self.local_names.push(format!("$arg{}", slot).into_bytes());
        }

        // entry statements come first, then state declarations
        let mut entry = Vec::new();
        while self.current_kind() != TokenKind::RBrace
            && !(self.current_kind() == TokenKind::Ident && self.current_text() == b"state")
        {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated machine body".into());
            }
            entry.push(self.parse_statement()?);
        }
        let mut states = Vec::new();
        while self.current_kind() == TokenKind::Ident && self.current_text() == b"state" {
            states.push(self.parse_state_decl()?);
        }
        if self.current_kind() != TokenKind::RBrace {
            return Err("alpha-onramp: parse error: expected '}' or 'state' in machine body".into());
        }
        self.bump(); // '}'
        Ok((entry, states))
    }

    // Scan from the current position (just inside the body '{') to the matching
    // '}', recording every top-level `state <name>` so transitions can refer to a
    // state declared later in the body.
    fn prescan_state_names(&mut self) -> Result<(), String> {
        let mut scan_position = self.position;
        let mut depth = 1i32;
        while depth > 0 {
            match self.tokens[scan_position].kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth -= 1,
                TokenKind::Eof => return Err("alpha-onramp: parse error: unterminated machine body".into()),
                TokenKind::Ident
                    if depth == 1 && token_text(&self.tokens[scan_position], self.source) == b"state" =>
                {
                    if self.tokens[scan_position + 1].kind == TokenKind::Ident {
                        self.state_names
                            .push(token_text(&self.tokens[scan_position + 1], self.source).to_vec());
                        // collect this state's parameter names: each `name : type` in the
                        // `(...)` (an Ident immediately followed by `:`); skips any `&mut self`.
                        let mut params: Vec<Vec<u8>> = Vec::new();
                        if self.tokens[scan_position + 2].kind == TokenKind::LParen {
                            let mut p = scan_position + 3;
                            while self.tokens[p].kind != TokenKind::RParen
                                && self.tokens[p].kind != TokenKind::Eof
                            {
                                if self.tokens[p].kind == TokenKind::Ident
                                    && self.tokens[p + 1].kind == TokenKind::Colon
                                {
                                    params.push(token_text(&self.tokens[p], self.source).to_vec());
                                }
                                p += 1;
                            }
                        }
                        self.state_param_names.push(params);
                    }
                }
                _ => {}
            }
            scan_position += 1;
        }
        Ok(())
    }

    fn parse_state_decl(&mut self) -> Result<Vec<Statement>, String> {
        self.bump(); // "state"
        self.expect(TokenKind::Ident)?; // name (already pre-scanned)
        self.expect(TokenKind::LParen)?;
        let mut params: Vec<Vec<u8>> = Vec::new();
        while self.current_kind() != TokenKind::RParen {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated state parameter list".into());
            }
            // a parameter is `name : type` (collect the name); skip `&mut self` etc.
            if self.current_kind() == TokenKind::Ident {
                let name = token_text(&self.tokens[self.position], self.source).to_vec();
                self.bump();
                if self.current_kind() == TokenKind::Colon {
                    self.bump(); // :
                    self.expect(TokenKind::Ident)?; // type (ignored — everything is i32)
                    params.push(name);
                    if self.current_kind() == TokenKind::Comma {
                        self.bump();
                    }
                }
            } else {
                self.bump(); // `&`, `mut`, etc.
            }
        }
        self.expect(TokenKind::RParen)?;
        // params resolve to this machine's shared state-arg slots while the body is parsed
        self.current_state_params = params;
        self.expect(TokenKind::LBrace)?;
        let mut statements = Vec::new();
        while self.current_kind() != TokenKind::RBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated state body".into());
            }
            statements.push(self.parse_statement()?);
        }
        self.bump(); // '}'
        self.current_state_params = Vec::new();
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        // `assert <cond>;` — a runtime contract: trap (like overflow/OOB) if cond is false.
        if self.current_kind() == TokenKind::Ident && self.current_text() == b"assert" {
            self.bump(); // assert
            let cond = self.parse_expression()?;
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            return Ok(Statement::Assert(cond));
        }
        if self.current_kind() == TokenKind::Ident && self.current_text() == b"let" {
            self.bump(); // let
            let name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
            self.expect(TokenKind::Colon)?;
            self.expect(TokenKind::Ident)?; // type (ignored at slice 2: everything is i32)
            // optional domain annotation `in <Domain>` (Omega's arithmetic-safety types). Only Wrapping
            // changes codegen (omit the overflow trap); Trapping is the default, others fall through to it.
            // `in <refinement>`: `in Wrapping`/`in Saturating` (Ident) = arithmetic domain; `in lo..hi` (Int) =
            // RANGE refinement (Omega's bounded type). A range let owes its initializer in [lo, hi) and, if the
            // local is never reassigned, carries that range downstream (a value-domain that flows through the body).
            let mut domain: u8 = 0; // 0 Trapping (default), 1 Wrapping, 2 Saturating
            let mut let_range: Option<(i32, i32)> = None;
            if self.current_kind() == TokenKind::Ident && self.current_text() == b"in" {
                self.bump(); // in
                if self.current_kind() == TokenKind::Int {
                    let lo = self.parse_int_token()?;
                    self.expect(TokenKind::Dot)?;
                    self.expect(TokenKind::Dot)?; // ..
                    let hi = self.parse_int_token()?;
                    let_range = Some((lo, hi));
                } else {
                    let dom = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                    if dom == b"Wrapping" {
                        domain = 1;
                    } else if dom == b"Saturating" {
                        domain = 2;
                    }
                }
            }
            self.expect(TokenKind::Eq)?;
            let init = self.parse_expression()?;
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            if self.find_local_index(&name).is_some() {
                return Err(format!(
                    "alpha-onramp: local '{}' already declared",
                    String::from_utf8_lossy(&name)
                ));
            }
            let local_index = self.local_names.len();
            self.local_names.push(name);
            if let Some((lo, hi)) = let_range {
                self.store_obligations.push((init, lo, hi)); // BoundedInitializer: init in [lo, hi)
                self.local_range_candidates.push((local_index, init)); // x == init downstream if never reassigned
            }
            return Ok(Statement::Let(local_index, init, domain));
        }
        if self.current_kind() == TokenKind::Ident && self.current_text() == b"transition" {
            self.bump(); // transition
            let subject = self.parse_expression()?;
            // if the subject is `self.<field>`, payload bindings read from <field>+8
            let subject_field_offset = match &self.expressions[subject] {
                Expr::SelfField(offset) => Some(*offset),
                _ => None,
            };
            self.expect(TokenKind::LBrace)?;
            let mut arms = Vec::new();
            while self.current_kind() != TokenKind::RBrace {
                if self.current_kind() == TokenKind::Eof {
                    return Err("alpha-onramp: parse error: unterminated transition".into());
                }
                // a pattern, plus an optional `{ x }` payload binding (only after a variant)
                let (pattern, binding): (Pattern, Vec<Vec<u8>>) = match self.current_kind() {
                    TokenKind::Int => {
                        let token = self.bump();
                        let text = std::str::from_utf8(token_text(&token, self.source)).unwrap();
                        let value: i64 = text
                            .parse()
                            .map_err(|_| format!("alpha-onramp: bad integer pattern '{}'", text))?;
                        (Pattern::Int(value as i32), Vec::new())
                    }
                    TokenKind::Ident => {
                        let token = self.bump();
                        match token_text(&token, self.source) {
                            b"_" => (Pattern::Wild, Vec::new()),
                            b"true" => (Pattern::Int(1), Vec::new()),
                            b"false" => (Pattern::Int(0), Vec::new()),
                            other => {
                                // EnumType::Variant pattern -> the variant's i32 tag,
                                // with an optional `{ x }` binding for the payload.
                                let other = other.to_vec();
                                if self.current_kind() == TokenKind::ColonColon {
                                    if let Some(enum_index) = self.find_enum(&other) {
                                        self.bump(); // ::
                                        let variant = token_text(
                                            &self.expect(TokenKind::Ident)?,
                                            self.source,
                                        )
                                        .to_vec();
                                        let tag = self
                                            .enum_variant_tag(enum_index, &variant)
                                            .ok_or_else(|| {
                                                format!(
                                                    "alpha-onramp: '{}' is not a variant of enum '{}'",
                                                    String::from_utf8_lossy(&variant),
                                                    String::from_utf8_lossy(&other)
                                                )
                                            })?;
                                        let mut bindings: Vec<Vec<u8>> = Vec::new();
                                        if self.current_kind() == TokenKind::LBrace {
                                            self.bump(); // {
                                            while self.current_kind() != TokenKind::RBrace {
                                                if self.current_kind() == TokenKind::Eof {
                                                    return Err("alpha-onramp: parse error: unterminated payload binding".into());
                                                }
                                                bindings.push(
                                                    token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec(),
                                                );
                                                if self.current_kind() == TokenKind::Comma {
                                                    self.bump();
                                                } else {
                                                    break;
                                                }
                                            }
                                            self.expect(TokenKind::RBrace)?;
                                            let pc = self.enum_variant_payload_count[enum_index][tag as usize];
                                            if bindings.len() != pc {
                                                return Err(format!(
                                                    "alpha-onramp: variant '{}' has {} payload field(s), bound {}",
                                                    String::from_utf8_lossy(&variant),
                                                    pc,
                                                    bindings.len()
                                                ));
                                            }
                                        }
                                        (Pattern::Int(tag), bindings)
                                    } else {
                                        return Err(format!(
                                            "alpha-onramp: unknown enum '{}' in transition pattern",
                                            String::from_utf8_lossy(&other)
                                        ));
                                    }
                                } else {
                                    return Err(format!(
                                        "alpha-onramp: unsupported transition pattern '{}'",
                                        String::from_utf8_lossy(&other)
                                    ));
                                }
                            }
                        }
                    }
                    other_kind => {
                        return Err(format!(
                            "alpha-onramp: parse error: expected a transition pattern, found {:?}",
                            other_kind
                        ))
                    }
                };
                self.expect(TokenKind::Arrow)?;
                let target_name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                let target = self.find_state_index(&target_name).ok_or_else(|| {
                    format!(
                        "alpha-onramp: transition to unknown state '{}'",
                        String::from_utf8_lossy(&target_name)
                    )
                })?;
                // a `Variant { a, b }` binding makes a,b resolve to the subject's payload
                // fields (field+8, +16, ...) while this arm's target args are parsed
                if !binding.is_empty() {
                    let base = subject_field_offset.ok_or_else(|| {
                        "alpha-onramp: a payload binding requires matching on `self.<field>`".to_string()
                    })? + 8;
                    let mut binds = Vec::new();
                    for (index, name) in binding.into_iter().enumerate() {
                        let node = self.add_expression(Expr::SelfField(base + (index as i32) * 8));
                        binds.push((name, node));
                    }
                    self.current_arm_bindings = binds;
                }
                self.expect(TokenKind::LParen)?;
                let mut args = Vec::new();
                while self.current_kind() != TokenKind::RParen {
                    if self.current_kind() == TokenKind::Eof {
                        self.current_arm_bindings.clear();
                        return Err("alpha-onramp: parse error: unterminated transition target".into());
                    }
                    args.push(self.parse_expression()?);
                    if self.current_kind() == TokenKind::Comma {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.current_arm_bindings.clear();
                self.expect(TokenKind::RParen)?;
                let expected = self.state_param_names[target].len();
                if args.len() != expected {
                    return Err(format!(
                        "alpha-onramp: state '{}' expects {} argument(s), got {}",
                        String::from_utf8_lossy(&target_name),
                        expected,
                        args.len()
                    ));
                }
                arms.push(TransitionArm { pattern, target, args });
            }
            self.bump(); // '}'
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            // Exhaustiveness: a transition whose subject is `self.<enum field>` must cover
            // every variant of that enum, or include a `_` arm. (Narrow but sound: only
            // direct enum-field subjects are checked; other subjects are unconstrained.)
            let subject_offset = match &self.expressions[subject] {
                Expr::SelfField(offset) => Some(*offset),
                _ => None,
            };
            if let Some(offset) = subject_offset {
                let enum_index = self.self_data_type.and_then(|self_type| {
                    // zero-size boundary fields can share an offset with a real slot, so
                    // match the enum field at this offset specifically (at most one exists).
                    self.data_field_maps[self_type]
                        .iter()
                        .find(|field| field.offset == offset && field.enum_index.is_some())
                        .and_then(|field| field.enum_index)
                });
                if let Some(enum_index) = enum_index {
                    let has_wild = arms.iter().any(|arm| matches!(arm.pattern, Pattern::Wild));
                    if !has_wild {
                        for tag in 0..self.enum_variant_lists[enum_index].len() {
                            let covered = arms
                                .iter()
                                .any(|arm| matches!(arm.pattern, Pattern::Int(value) if value == tag as i32));
                            if !covered {
                                return Err(format!(
                                    "alpha-onramp: non-exhaustive transition over enum '{}': missing variant '{}' (add it or a `_` arm)",
                                    String::from_utf8_lossy(&self.enum_names[enum_index]),
                                    String::from_utf8_lossy(&self.enum_variant_lists[enum_index][tag])
                                ));
                            }
                        }
                    }
                }
            }
            return Ok(Statement::Transition(subject, arms));
        }
        if self.current_kind() == TokenKind::Ident && self.current_text() == b"return" {
            self.bump(); // return
            let value = self.parse_expression()?;
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            return Ok(Statement::Return(value));
        }
        // A statement starting with an identifier is either a place assignment
        // (`x = e`, `self.f = e`) or a call (`recv.op(args)`). Collect the dotted
        // path, then branch on the terminator (`=` vs `(`).
        let mut segments = vec![token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec()];
        while self.current_kind() == TokenKind::Dot {
            self.bump();
            segments.push(token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec());
        }
        if self.current_kind() == TokenKind::LBracket {
            // array-element assignment: self.<array>[index] = value
            if segments[0] != b"self" || segments.len() != 2 {
                return Err("alpha-onramp: only `self.<array>[i] = ...` is supported (slice 7b)".into());
            }
            let (offset, count, element_bytes) = self.self_array_field(&segments[1])?;
            self.bump(); // [
            let index = self.parse_expression()?;
            self.expect(TokenKind::RBracket)?;
            self.expect(TokenKind::Eq)?;
            let value = self.parse_expression()?;
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            return Ok(Statement::StoreSelfIndex(offset, count, element_bytes, index, value));
        }
        if self.current_kind() == TokenKind::Eq {
            self.bump(); // =
            // enum payload construction: `self.f = E::V(arg)` writes the tag at f and the
            // payload at f+8 (desugars to a Block of field stores).
            if segments[0] == b"self" && segments.len() == 2 {
                if let Some(block) = self.try_parse_enum_construction(&segments[1])? {
                    if self.current_kind() == TokenKind::Semi {
                        self.bump();
                    }
                    return Ok(block);
                }
            }
            let value = self.parse_expression()?;
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            if segments[0] == b"self" {
                if segments.len() < 2 {
                    return Err("alpha-onramp: `self = ...` is not assignable".into());
                }
                // single field via the scalar resolver; nested `self.a.b = ...` via the path resolver
                let (offset, domain) = if segments.len() == 2 {
                    (self.self_field_offset(&segments[1])?, self.self_field_domain(&segments[1]))
                } else {
                    self.self_field_path(&segments[1..])?
                };
                // Range-refined field (`f: i32 in lo..hi`): record the BOUNDED-STORE obligation that the
                // stored value lies in [lo, hi) (Omega's BoundedAssignment obligation), for static discharge.
                if segments.len() == 2 {
                    if let Some((lo, hi)) = self.self_field_range(&segments[1]) {
                        self.store_obligations.push((value, lo, hi));
                    }
                }
                return Ok(Statement::StoreSelfField(offset, value, domain));
            }
            if segments.len() != 1 {
                return Err("alpha-onramp: local field assignment is unsupported (slice 7a)".into());
            }
            let local_index = self.find_local_index(&segments[0]).ok_or_else(|| {
                format!(
                    "alpha-onramp: assignment to undeclared local '{}'",
                    String::from_utf8_lossy(&segments[0])
                )
            })?;
            return Ok(Statement::Assign(local_index, value));
        }
        // method call statement: self.<method>(args)
        if segments.len() == 2 && segments[0] == b"self" && self.current_kind() == TokenKind::LParen {
            if let Some(machine_index) = self.find_machine_index(&segments[1]) {
                let expr = self.parse_call(machine_index, true)?;
                if self.current_kind() == TokenKind::Semi {
                    self.bump();
                }
                return Ok(Statement::Eval(expr));
            }
        }
        // free call statement: <machine>(args)
        if segments.len() == 1 && self.current_kind() == TokenKind::LParen {
            if let Some(machine_index) = self.find_machine_index(&segments[0]) {
                let expr = self.parse_call(machine_index, false)?;
                if self.current_kind() == TokenKind::Semi {
                    self.bump();
                }
                return Ok(Statement::Eval(expr));
            }
        }
        // otherwise a boundary op call: the last path segment selects the op
        self.expect(TokenKind::LParen)?;
        let last_segment = segments.last().unwrap().clone();
        let statement = match last_segment.as_slice() {
            b"exit_process" => Statement::Exit(self.parse_expression()?),
            b"write_byte" => {
                self.uses_host_io = true;
                self.current_machine_makes_call = true;
                Statement::WriteByte(self.parse_expression()?)
            }
            b"write_line" => {
                self.uses_host_io = true;
                self.current_machine_makes_call = true;
                let string_token = self.expect(TokenKind::Str)?;
                let mut bytes = token_text(&string_token, self.source).to_vec();
                bytes.push(b'\n'); // write_line appends a newline
                let string_index = self.strings.len();
                self.strings.push(bytes);
                Statement::WriteLine(string_index)
            }
            other => {
                return Err(format!(
                    "alpha-onramp: unsupported call '{}' (supported: exit_process, write_line, write_byte)",
                    String::from_utf8_lossy(other)
                ))
            }
        };
        self.expect(TokenKind::RParen)?;
        if self.current_kind() == TokenKind::Semi {
            self.bump();
        }
        Ok(statement)
    }

    fn parse_expression(&mut self) -> Result<usize, String> {
        self.parse_binary(0)
    }

    // Parse `(arg, arg, ...)` after a machine name; record args in the flat
    // call_args arena. The leading '(' has not been consumed yet. `pass_self` means
    // a method call `self.m(..)` — self occupies rcx, so only 3 register args remain.
    fn parse_call(&mut self, machine_index: usize, pass_self: bool) -> Result<usize, String> {
        self.current_machine_makes_call = true;
        self.expect(TokenKind::LParen)?;
        let mut arg_nodes = Vec::new();
        while self.current_kind() != TokenKind::RParen {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated call arguments".into());
            }
            arg_nodes.push(self.parse_expression()?);
            if self.current_kind() == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        let max_args = if pass_self { 3 } else { 4 };
        if arg_nodes.len() > max_args {
            return Err(format!(
                "alpha-onramp: call has {} args; the on-ramp ABI supports at most {} here",
                arg_nodes.len(),
                max_args
            ));
        }
        let args_start = self.call_args.len();
        let arg_count = arg_nodes.len();
        self.call_args.extend(arg_nodes);
        let node = if pass_self {
            Expr::SelfCall(machine_index, args_start, arg_count)
        } else {
            Expr::Call(machine_index, args_start, arg_count)
        };
        Ok(self.add_expression(node))
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Result<usize, String> {
        let mut lhs = self.parse_primary()?;
        loop {
            let (op, precedence) = match self.current_kind() {
                TokenKind::Lt => (BinaryOp::Lt, 1u8),
                TokenKind::Gt => (BinaryOp::Gt, 1),
                TokenKind::Le => (BinaryOp::Le, 1),
                TokenKind::Ge => (BinaryOp::Ge, 1),
                TokenKind::EqEq => (BinaryOp::EqEq, 1),
                TokenKind::Ne => (BinaryOp::Ne, 1),
                TokenKind::Plus => (BinaryOp::Add, 2),
                TokenKind::Minus => (BinaryOp::Sub, 2),
                TokenKind::Star => (BinaryOp::Mul, 3),
                TokenKind::Slash => (BinaryOp::Div, 3),
                TokenKind::Percent => (BinaryOp::Rem, 3),
                // bitwise ops share one level, looser than comparison (parens to mix & | ^)
                TokenKind::Amp => (BinaryOp::BitAnd, 0),
                TokenKind::Pipe => (BinaryOp::BitOr, 0),
                TokenKind::Caret => (BinaryOp::BitXor, 0),
                TokenKind::Shl => (BinaryOp::Shl, 0),
                TokenKind::Shr => (BinaryOp::Shr, 0),
                _ => break,
            };
            if precedence < min_precedence {
                break;
            }
            self.bump(); // operator
            let rhs = self.parse_binary(precedence + 1)?; // left-associative
            lhs = self.add_expression(Expr::Binary(op, lhs, rhs));
        }
        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<usize, String> {
        match self.current_kind() {
            TokenKind::Minus => {
                // unary minus: -x desugars to (0 - x), reusing Sub's overflow trap (so
                // negating INT_MIN traps). Recurse into a primary so unary binds tightest.
                self.bump();
                let operand = self.parse_primary()?;
                let zero = self.add_expression(Expr::Int(0));
                Ok(self.add_expression(Expr::Binary(BinaryOp::Sub, zero, operand)))
            }
            TokenKind::Int => {
                let token = self.bump();
                let text = std::str::from_utf8(token_text(&token, self.source)).unwrap();
                let value: i64 = text
                    .parse()
                    .map_err(|_| format!("alpha-onramp: bad integer literal '{}'", text))?;
                if value > i32::MAX as i64 {
                    return Err(format!("alpha-onramp: integer literal {} out of i32 range", value));
                }
                Ok(self.add_expression(Expr::Int(value as i32)))
            }
            TokenKind::Ident => {
                let token = self.bump();
                let name = token_text(&token, self.source).to_vec();
                if name == b"self" {
                    self.expect(TokenKind::Dot)?;
                    let field = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                    if self.current_kind() == TokenKind::LParen {
                        // method call self.m(args) — passes self
                        if let Some(machine_index) = self.find_machine_index(&field) {
                            return self.parse_call(machine_index, true);
                        }
                        return Err(format!(
                            "alpha-onramp: 'self.{}(..)' is not a method",
                            String::from_utf8_lossy(&field)
                        ));
                    }
                    if self.current_kind() == TokenKind::LBracket {
                        let (offset, count, element_bytes) = self.self_array_field(&field)?;
                        self.bump(); // [
                        let index = self.parse_expression()?;
                        self.expect(TokenKind::RBracket)?;
                        return Ok(self.add_expression(Expr::SelfIndex(offset, count, element_bytes, index)));
                    }
                    if self.current_kind() == TokenKind::Dot {
                        // nested read: self.a.b.c… — walk the inlined-struct path to a flat offset
                        let mut path = vec![field];
                        while self.current_kind() == TokenKind::Dot {
                            self.bump(); // .
                            path.push(token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec());
                        }
                        let (offset, _domain) = self.self_field_path(&path)?;
                        return Ok(self.add_expression(Expr::SelfField(offset)));
                    }
                    let offset = self.self_field_offset(&field)?;
                    return Ok(self.add_expression(Expr::SelfField(offset)));
                }
                if name == b"read_byte" && self.current_kind() == TokenKind::LParen {
                    self.bump(); // (
                    self.expect(TokenKind::RParen)?;
                    self.uses_host_io = true;
                    self.current_machine_makes_call = true;
                    return Ok(self.add_expression(Expr::ReadByte));
                }
                // min(a, b) / max(a, b) builtins — Omega's clamping idiom `max(lo, min(v, hi))`. Two-argument
                // intrinsics (not machines), lowered to a branchless cmp+csel; no overflow, so no domain/trap.
                if (name == b"min" || name == b"max") && self.current_kind() == TokenKind::LParen {
                    self.bump(); // (
                    let a = self.parse_expression()?;
                    self.expect(TokenKind::Comma)?;
                    let b = self.parse_expression()?;
                    self.expect(TokenKind::RParen)?;
                    let node = if name == b"min" { Expr::Min(a, b) } else { Expr::Max(a, b) };
                    return Ok(self.add_expression(node));
                }
                if self.current_kind() == TokenKind::ColonColon {
                    if let Some(enum_index) = self.find_enum(&name) {
                        self.bump(); // ::
                        let variant = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                        let tag = self.enum_variant_tag(enum_index, &variant).ok_or_else(|| {
                            format!(
                                "alpha-onramp: '{}' is not a variant of enum '{}'",
                                String::from_utf8_lossy(&variant),
                                String::from_utf8_lossy(&name)
                            )
                        })?;
                        // tag-only enum value IS its i32 tag
                        return Ok(self.add_expression(Expr::Int(tag)));
                    }
                }
                if self.current_kind() == TokenKind::LParen {
                    return match self.find_machine_index(&name) {
                        Some(machine_index) => self.parse_call(machine_index, false),
                        None => Err(format!(
                            "alpha-onramp: call to unknown machine '{}'",
                            String::from_utf8_lossy(&name)
                        )),
                    };
                }
                // `Variant { a, b }` payload bindings resolve to their synthesized payload reads
                for (bind_name, node) in &self.current_arm_bindings {
                    if bind_name.as_slice() == name.as_slice() {
                        return Ok(*node);
                    }
                }
                match self.find_local_index(&name) {
                    Some(local_index) => Ok(self.add_expression(Expr::Local(local_index))),
                    None => Err(format!(
                        "alpha-onramp: unknown identifier '{}'",
                        String::from_utf8_lossy(&name)
                    )),
                }
            }
            TokenKind::LParen => {
                self.bump();
                let expression_node = self.parse_expression()?;
                self.expect(TokenKind::RParen)?;
                Ok(expression_node)
            }
            other_kind => Err(format!(
                "alpha-onramp: parse error: expected an expression, found {:?}",
                other_kind
            )),
        }
    }
}
