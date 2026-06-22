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
enum FieldKind {
    Scalar,      // i32-width value, occupies 8 bytes
    Boundary,    // a capability handle (e.g. Console): zero runtime storage
    Data(usize), // a nested data type (occupies that type's size)
}

struct DataFieldInfo {
    name: Vec<u8>,
    offset: i32, // byte offset within the struct
    kind: FieldKind,
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
    local_names: Vec<Vec<u8>>,        // locals of the machine being parsed (params first)
    state_names: Vec<Vec<u8>>,        // pre-scanned state names of the machine being parsed
    self_data_type: Option<usize>,    // data type of `self` in the machine being parsed
    current_machine_makes_call: bool, // set while parsing a machine that calls out
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
            local_names: Vec::new(),
            state_names: Vec::new(),
            self_data_type: None,
            current_machine_makes_call: false,
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
    fn find_local_index(&self, name: &[u8]) -> Option<usize> {
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
        let mut offset = 0i32;
        while self.current_kind() != TokenKind::RBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated data body".into());
            }
            let field_name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
            self.expect(TokenKind::Colon)?;
            if self.current_kind() == TokenKind::Amp {
                return Err("alpha-onramp: ref/slice data fields are unsupported (slice 7a)".into());
            }
            let type_name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
            let kind = self.classify_field_type(&type_name)?;
            let size = match kind {
                FieldKind::Scalar => 8,
                FieldKind::Boundary => 0,
                FieldKind::Data(index) => self.data_type_sizes[index],
            };
            fields.push(DataFieldInfo { name: field_name, offset, kind });
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
        self.data_type_names.push(name);
        self.data_type_sizes.push(offset);
        self.data_field_maps.push(fields);
        Ok(())
    }

    fn classify_field_type(&self, type_name: &[u8]) -> Result<FieldKind, String> {
        if self.boundary_names.iter().any(|boundary| boundary.as_slice() == type_name) {
            return Ok(FieldKind::Boundary);
        }
        if let Some(index) = self.find_data_type(type_name) {
            return Ok(FieldKind::Data(index));
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
        self.self_data_type = None;
        self.current_machine_makes_call = false;
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
                {
                    if self.current_kind() == TokenKind::Eof {
                        return Err("alpha-onramp: parse error: unterminated parameter type".into());
                    }
                    self.bump(); // skip the type (slice 6: everything is i32-width)
                }
                self.local_names.push(param_name);
                param_count += 1;
            }
            if self.current_kind() == TokenKind::Comma {
                self.bump();
            }
        }
        self.expect(TokenKind::RParen)?;

        if self.current_kind() == TokenKind::Arrow {
            self.bump();
            while self.current_kind() != TokenKind::LBrace {
                if self.current_kind() == TokenKind::Eof {
                    return Err("alpha-onramp: parse error: unterminated return type".into());
                }
                self.bump(); // skip the return type
            }
        }

        // `self`'s data type is the receiver (the name path's first segment), if any
        let self_data_type = if has_self { self.find_data_type(&receiver_type) } else { None };
        self.self_data_type = self_data_type;
        self.machine_self_types.push(self_data_type);

        let (entry, states) = self.parse_machine_body()?;
        Ok(Machine {
            param_count,
            local_count: self.local_names.len(),
            makes_call: self.current_machine_makes_call,
            has_self,
            entry,
            states,
        })
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
        while self.current_kind() != TokenKind::RParen {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated state parameter list".into());
            }
            self.bump(); // skip params (e.g. &mut self)
        }
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;
        let mut statements = Vec::new();
        while self.current_kind() != TokenKind::RBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated state body".into());
            }
            statements.push(self.parse_statement()?);
        }
        self.bump(); // '}'
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        if self.current_kind() == TokenKind::Ident && self.current_text() == b"let" {
            self.bump(); // let
            let name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
            self.expect(TokenKind::Colon)?;
            self.expect(TokenKind::Ident)?; // type (ignored at slice 2: everything is i32)
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
            return Ok(Statement::Let(local_index, init));
        }
        if self.current_kind() == TokenKind::Ident && self.current_text() == b"transition" {
            self.bump(); // transition
            let subject = self.parse_expression()?;
            self.expect(TokenKind::LBrace)?;
            let mut arms = Vec::new();
            while self.current_kind() != TokenKind::RBrace {
                if self.current_kind() == TokenKind::Eof {
                    return Err("alpha-onramp: parse error: unterminated transition".into());
                }
                let pattern = match self.current_kind() {
                    TokenKind::Int => {
                        let token = self.bump();
                        let text = std::str::from_utf8(token_text(&token, self.source)).unwrap();
                        let value: i64 = text
                            .parse()
                            .map_err(|_| format!("alpha-onramp: bad integer pattern '{}'", text))?;
                        Pattern::Int(value as i32)
                    }
                    TokenKind::Ident => {
                        let token = self.bump();
                        match token_text(&token, self.source) {
                            b"_" => Pattern::Wild,
                            b"true" => Pattern::Int(1),
                            b"false" => Pattern::Int(0),
                            other => {
                                return Err(format!(
                                    "alpha-onramp: unsupported transition pattern '{}'",
                                    String::from_utf8_lossy(other)
                                ))
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
                self.expect(TokenKind::LParen)?;
                while self.current_kind() != TokenKind::RParen {
                    if self.current_kind() == TokenKind::Eof {
                        return Err("alpha-onramp: parse error: unterminated transition target".into());
                    }
                    self.bump(); // skip target args (slice 4b: parameterless states)
                }
                self.expect(TokenKind::RParen)?;
                let target = self.find_state_index(&target_name).ok_or_else(|| {
                    format!(
                        "alpha-onramp: transition to unknown state '{}'",
                        String::from_utf8_lossy(&target_name)
                    )
                })?;
                arms.push(TransitionArm { pattern, target });
            }
            self.bump(); // '}'
            if self.current_kind() == TokenKind::Semi {
                self.bump();
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
        if self.current_kind() == TokenKind::Eq {
            self.bump(); // =
            let value = self.parse_expression()?;
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            if segments[0] == b"self" {
                if segments.len() != 2 {
                    return Err("alpha-onramp: only `self.<field> = ...` is supported (slice 7a)".into());
                }
                let offset = self.self_field_offset(&segments[1])?;
                return Ok(Statement::StoreSelfField(offset, value));
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
        // call statement: the last path segment selects the boundary op
        self.expect(TokenKind::LParen)?;
        let last_segment = segments.last().unwrap().clone();
        let statement = match last_segment.as_slice() {
            b"exit_process" => Statement::Exit(self.parse_expression()?),
            b"write_line" => {
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
                    "alpha-onramp: unsupported call '{}' (supported: exit_process, write_line)",
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
        self.parse_binary(1)
    }

    // Parse `(arg, arg, ...)` after a machine name; record args in the flat
    // call_args arena. The leading '(' has not been consumed yet.
    fn parse_call(&mut self, machine_index: usize) -> Result<usize, String> {
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
        if arg_nodes.len() > 4 {
            return Err(format!(
                "alpha-onramp: call has {} args; the on-ramp ABI supports at most 4 (slice 6)",
                arg_nodes.len()
            ));
        }
        let args_start = self.call_args.len();
        let arg_count = arg_nodes.len();
        self.call_args.extend(arg_nodes);
        Ok(self.add_expression(Expr::Call(machine_index, args_start, arg_count)))
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
                    let offset = self.self_field_offset(&field)?;
                    return Ok(self.add_expression(Expr::SelfField(offset)));
                }
                if self.current_kind() == TokenKind::LParen {
                    return match self.find_machine_index(&name) {
                        Some(machine_index) => self.parse_call(machine_index),
                        None => Err(format!(
                            "alpha-onramp: call to unknown machine '{}'",
                            String::from_utf8_lossy(&name)
                        )),
                    };
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
