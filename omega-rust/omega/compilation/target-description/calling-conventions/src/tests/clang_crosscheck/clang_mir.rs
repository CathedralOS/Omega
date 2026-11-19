/*
This module reads where clang put each argument and result byte. clang lowers
each caller `c<k>` at `-O0` and stops after instruction selection
(`-stop-after=finalize-isel`), so the MIR still shows plain loads from the
argument globals, the copies into argument registers, the stores to the
outgoing stack area, and the store of the returned value. The tracer follows
each byte of `g<k>_<i>` and `r<k>` through those instructions.

It only understands the instructions clang 18 emits for these callers on the
four hosted targets. Any other instruction that touches a tracked value makes
the signature a `HarnessFailure` instead of a guess, so a newer clang shows up
as an explained harness failure, never as a false mismatch.
*/

use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct HarnessFailure(pub(super) String);

fn fail<T>(reason: impl Into<String>) -> Result<T, HarnessFailure> {
    Err(HarnessFailure(reason.into()))
}

/// The memory a pointer value addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Base {
    Global(String, i64),
    StackObject(u32, i64),
    OutgoingArea(i64),
}

impl Base {
    fn plus(&self, delta: i64) -> Self {
        match self {
            Self::Global(name, offset) => Self::Global(name.clone(), offset + delta),
            Self::StackObject(object, offset) => Self::StackObject(*object, offset + delta),
            Self::OutgoingArea(offset) => Self::OutgoingArea(offset + delta),
        }
    }
}

/// Where one byte of a value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    Global(String, i64),
    Register(Option<String>, i64),
    StackMemory(u32, i64),
    Pointer(Base),
}

type Bytes = BTreeMap<i64, Option<Source>>;

#[derive(Debug, Clone, Default)]
struct Value {
    bytes: Bytes,
    pointer: Option<Base>,
    constant: Option<i64>,
}

impl Value {
    fn bytes(bytes: Bytes) -> Self {
        Self {
            bytes,
            ..Self::default()
        }
    }

    fn pointer(base: Base) -> Self {
        Self {
            pointer: Some(base),
            ..Self::default()
        }
    }

    fn byte_source(&self, byte: i64) -> Option<Source> {
        match &self.pointer {
            Some(base) => Some(Source::Pointer(base.clone())),
            None => self.bytes.get(&byte).cloned().flatten(),
        }
    }
}

/// The caller functions `c<k>` in a MIR file, by `k`.
pub(super) fn caller_bodies(mir: &str) -> BTreeMap<usize, Vec<&str>> {
    let mut bodies = BTreeMap::new();
    let mut lines = mir.lines();
    while let Some(line) = lines.next() {
        let Some(name) = line.strip_prefix("name:") else {
            continue;
        };
        let Some(k) = name.trim().strip_prefix('c').and_then(|k| k.parse().ok()) else {
            continue;
        };
        let body = lines
            .by_ref()
            .take_while(|line| !line.starts_with("---") && !line.starts_with("..."))
            .skip_while(|line| !line.starts_with("body:"))
            .skip(1)
            .collect();
        bodies.insert(k, body);
    }
    bodies
}

fn canonical_register(register: &str) -> Option<String> {
    const X86: [(&str, [&str; 4]); 6] = [
        ("rax", ["rax", "eax", "ax", "al"]),
        ("rbx", ["rbx", "ebx", "bx", "bl"]),
        ("rcx", ["rcx", "ecx", "cx", "cl"]),
        ("rdx", ["rdx", "edx", "dx", "dl"]),
        ("rsi", ["rsi", "esi", "si", "sil"]),
        ("rdi", ["rdi", "edi", "di", "dil"]),
    ];
    for (full, names) in X86 {
        if names.contains(&register) {
            return Some(full.to_string());
        }
    }
    if let Some(rest) = register.strip_prefix('r') {
        let digits = rest.trim_end_matches(['d', 'w', 'b']);
        if rest.len() - digits.len() <= 1
            && let Ok(index @ 8..=15) = digits.parse::<u8>()
        {
            return Some(format!("r{index}"));
        }
    }
    if register.starts_with("xmm") {
        return Some(register.to_string());
    }
    let (bank, digits) = register.split_at(1);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match bank {
        "w" | "x" => Some(format!("x{digits}")),
        "b" | "h" | "s" | "d" | "q" => Some(format!("v{digits}")),
        _ => None,
    }
}

fn access_size(op: &str) -> Result<i64, HarnessFailure> {
    const X86: [(&str, i64); 8] = [
        ("MOV8", 1),
        ("MOV16", 2),
        ("MOV32", 4),
        ("MOV64", 8),
        ("MOVSS", 4),
        ("MOVSD", 8),
        ("MOVLPD", 8),
        ("MOVSX64rm32", 4),
    ];
    for (prefix, size) in X86 {
        if op.starts_with(prefix) {
            return Ok(size);
        }
    }
    if ["MOVUPS", "MOVAPS", "MOVUPD", "MOVAPD"]
        .iter()
        .any(|prefix| op.starts_with(prefix))
    {
        return Ok(16);
    }
    if let Some(rest) = op
        .strip_prefix("MOVSX")
        .or_else(|| op.strip_prefix("MOVZX"))
    {
        let rest = rest
            .strip_prefix("32")
            .or_else(|| rest.strip_prefix("64"))
            .unwrap_or("");
        if rest.starts_with("rm8") {
            return Ok(1);
        }
        if rest.starts_with("rm16") {
            return Ok(2);
        }
    }
    let arm = ["LDUR", "STUR", "LDR", "STR"]
        .iter()
        .find_map(|prefix| op.strip_prefix(prefix));
    if let Some(rest) = arm {
        for (forms, size) in [
            (["B", "SB"], 1),
            (["H", "SH"], 2),
            (["W", "S"], 4),
            (["X", "D"], 8),
            (["Q", "Q"], 16),
        ] {
            if forms.iter().any(|form| rest.starts_with(form)) {
                return Ok(size);
            }
        }
    }
    fail(format!("size of {op}"))
}

/// `@name` or `@name + 8`, optionally after `target-flags(...)`.
fn symbol(token: &str) -> Option<(String, i64)> {
    let token = match token.strip_prefix("target-flags(") {
        Some(rest) => rest.split_once(") ")?.1,
        None => token,
    };
    let token = token.strip_prefix('@')?;
    let (name, offset) = match token.split_once(" + ") {
        Some((name, offset)) => (name, offset.parse().ok()?),
        None => (token, 0),
    };
    name.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        .then(|| (name.to_string(), offset))
}

fn integer(token: &str) -> Result<i64, HarnessFailure> {
    token
        .parse()
        .map_err(|_| HarnessFailure(format!("integer operand {token}")))
}

fn operand<'a>(ops: &[&'a str], index: usize) -> Result<&'a str, HarnessFailure> {
    ops.get(index)
        .copied()
        .ok_or_else(|| HarnessFailure(format!("missing operand {index}")))
}

/// Drops MIR operand flags that do not change which value flows.
fn without_flags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        let at_word_start =
            out.is_empty() || !out.ends_with(|c: char| c.is_alphanumeric() || c == '_');
        let flag = ["killed ", "undef ", "dead ", "renamable "]
            .iter()
            .find(|flag| at_word_start && rest.starts_with(**flag));
        if let Some(flag) = flag {
            rest = &rest[flag.len()..];
            continue;
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// What clang's caller left at the call: the argument registers, the
/// outgoing stack area, the caller's stack objects and the globals it wrote.
struct CallState {
    argument_registers: BTreeMap<String, Value>,
    outgoing_area: BTreeMap<i64, Option<Source>>,
    stack_objects: BTreeMap<u32, BTreeMap<i64, Option<Source>>>,
    globals_written: BTreeMap<(String, i64), Option<Source>>,
}

struct Tracer {
    values: BTreeMap<String, Value>,
    stack_objects: BTreeMap<u32, BTreeMap<i64, Option<Source>>>,
    outgoing_area: BTreeMap<i64, Option<Source>>,
    argument_registers: BTreeMap<String, Value>,
    globals_written: BTreeMap<(String, i64), Option<Source>>,
}

impl Tracer {
    fn value(&self, name: &str) -> Result<&Value, HarnessFailure> {
        self.values
            .get(name)
            .ok_or_else(|| HarnessFailure(format!("untracked value {name}")))
    }

    fn base_of(&self, token: &str) -> Option<Base> {
        if let Some(base) = self.values.get(token).and_then(|v| v.pointer.clone()) {
            return Some(base);
        }
        if let Some(object) = token.strip_prefix("%stack.") {
            return object.parse().ok().map(|o| Base::StackObject(o, 0));
        }
        matches!(token, "$rsp" | "$sp").then_some(Base::OutgoingArea(0))
    }

    fn read(&self, base: &Base, offset: i64, size: i64) -> Value {
        let bytes = (0..size)
            .map(|j| {
                let source = match base.plus(offset + j) {
                    Base::Global(name, at) => Some(Source::Global(name, at)),
                    Base::StackObject(object, at) => self
                        .stack_objects
                        .get(&object)
                        .and_then(|bytes| bytes.get(&at).cloned())
                        .unwrap_or(Some(Source::StackMemory(object, at))),
                    Base::OutgoingArea(_) => None,
                };
                (j, source)
            })
            .collect();
        Value::bytes(bytes)
    }

    fn write(&mut self, base: &Base, offset: i64, size: i64, value: &Value) {
        for j in 0..size {
            let source = value.byte_source(j);
            match base.plus(offset + j) {
                Base::StackObject(object, at) => {
                    self.stack_objects
                        .entry(object)
                        .or_default()
                        .insert(at, source);
                }
                Base::OutgoingArea(at) => {
                    self.outgoing_area.insert(at, source);
                }
                Base::Global(name, at) => {
                    self.globals_written.insert((name, at), source);
                }
            }
        }
    }

    /// Runs `memcpy(dst, src, n)` with constant `n` over the tracked memory.
    fn memcpy(&mut self) -> Result<(), HarnessFailure> {
        for [dst, src, len] in [
            ["x0", "x1", "x2"],
            ["rdi", "rsi", "rdx"],
            ["rcx", "rdx", "r8"],
        ] {
            let registers = &self.argument_registers;
            let (Some(d), Some(s), Some(n)) = (
                registers.get(dst).and_then(|v| v.pointer.clone()),
                registers.get(src).and_then(|v| v.pointer.clone()),
                registers.get(len).and_then(|v| v.constant),
            ) else {
                continue;
            };
            let copied = self.read(&s, 0, n);
            self.write(&d, 0, n, &copied);
            self.argument_registers.clear();
            return Ok(());
        }
        fail("memcpy operands")
    }

    fn step(
        &mut self,
        dst: Option<&str>,
        op: &str,
        ops: &[&str],
        line: &str,
    ) -> Result<(), HarnessFailure> {
        let need_dst = || dst.ok_or_else(|| HarnessFailure(format!("no destination: {line}")));
        match op {
            "MOVi64imm" | "MOVi32imm" | "MOV32ri64" | "MOV32ri" | "MOV64ri" => {
                let value = Value {
                    constant: Some(integer(operand(ops, 0)?)?),
                    ..Value::default()
                };
                self.values.insert(need_dst()?.to_string(), value);
                return Ok(());
            }
            "DUPi64" | "DUPi32" => {
                let width = if op == "DUPi64" { 8 } else { 4 };
                let shift = integer(operand(ops, 1)?)? * width;
                let bytes = self
                    .value(operand(ops, 0)?)?
                    .bytes
                    .iter()
                    .filter(|(j, _)| (0..width).contains(&(**j - shift)))
                    .map(|(j, s)| (j - shift, s.clone()))
                    .collect();
                self.values
                    .insert(need_dst()?.to_string(), Value::bytes(bytes));
                return Ok(());
            }
            "INSvi64gpr" | "INSvi64lane" | "INSvi32lane" => {
                let mut bytes = self.value(operand(ops, 0)?)?.bytes.clone();
                let lane = integer(operand(ops, 1)?)?;
                let width = if op.contains("32") { 4 } else { 8 };
                let source = &self.value(operand(ops, 2)?)?.bytes;
                let source_lane = match ops.get(3) {
                    Some(lane) => integer(lane)?,
                    None => 0,
                };
                for j in 0..width {
                    let byte = source.get(&(source_lane * width + j)).cloned().flatten();
                    bytes.insert(lane * width + j, byte);
                }
                self.values
                    .insert(need_dst()?.to_string(), Value::bytes(bytes));
                return Ok(());
            }
            "COPY" => return self.copy(need_dst()?, operand(ops, 0)?, line),
            "LOADgot" => {
                let Some((name, offset)) = symbol(operand(ops, 0)?) else {
                    return fail(format!("got symbol {line}"));
                };
                self.values.insert(
                    need_dst()?.to_string(),
                    Value::pointer(Base::Global(name, offset)),
                );
                return Ok(());
            }
            "SUBREG_TO_REG" => {
                let value = self.value(operand(ops, 1)?)?.clone();
                self.values.insert(need_dst()?.to_string(), value);
                return Ok(());
            }
            "INSERT_SUBREG" => {
                if self
                    .values
                    .get(operand(ops, 0)?)
                    .is_some_and(|v| !v.bytes.is_empty())
                {
                    return fail("insert into live value");
                }
                let value = self.value(operand(ops, 1)?)?.clone();
                self.values.insert(need_dst()?.to_string(), value);
                return Ok(());
            }
            "ADD64rr_DB" | "OR64rr" | "ADD32rr_DB" | "OR32rr" | "ORRXrr" | "ORRWrr" => {
                let mut bytes = self.value(operand(ops, 0)?)?.bytes.clone();
                bytes.extend(self.value(operand(ops, 1)?)?.bytes.clone());
                self.values
                    .insert(need_dst()?.to_string(), Value::bytes(bytes));
                return Ok(());
            }
            "LEA64r" => {
                let pointer = if operand(ops, 0)? == "$rip" {
                    let Some((name, offset)) = symbol(operand(ops, 3)?) else {
                        return fail(format!("rip lea {line}"));
                    };
                    Base::Global(name, offset)
                } else {
                    let Some(base) = self.base_of(operand(ops, 0)?) else {
                        return fail(format!("lea base {line}"));
                    };
                    base.plus(integer(operand(ops, 3)?)?)
                };
                self.values
                    .insert(need_dst()?.to_string(), Value::pointer(pointer));
                return Ok(());
            }
            "ADDXri" => {
                let base = self.base_of(operand(ops, 0)?);
                let (Some(base), "0") = (base, operand(ops, 2)?) else {
                    return fail(format!("addxri {line}"));
                };
                let pointer = base.plus(integer(operand(ops, 1)?)?);
                self.values
                    .insert(need_dst()?.to_string(), Value::pointer(pointer));
                return Ok(());
            }
            _ => {}
        }
        if is_extend_move(op) {
            let value = self.value(operand(ops, 0)?)?.clone();
            self.values.insert(need_dst()?.to_string(), value);
            return Ok(());
        }
        if let Some(value) = self.byte_shift(op, ops)? {
            self.values.insert(need_dst()?.to_string(), value);
            return Ok(());
        }
        let memory = op.starts_with("MOV")
            || op.starts_with("LDR")
            || op.starts_with("LDUR")
            || op.starts_with("STR")
            || op.starts_with("STUR");
        let load_form = op.starts_with("LDR")
            || op.starts_with("LDUR")
            || (op.starts_with("MOV") && op[op.len().saturating_sub(8)..].contains("rm"));
        if memory
            && load_form
            && let Some(dst) = dst
        {
            return self.load(dst, op, ops, line);
        }
        if memory && dst.is_none() && !op.starts_with("LD") {
            return self.store(op, ops, line);
        }
        fail(format!("unknown op {op}: {line}"))
    }

    fn copy(&mut self, dst: &str, src: &str, line: &str) -> Result<(), HarnessFailure> {
        if let Some(register) = dst.strip_prefix('$') {
            let Some(register) = canonical_register(register) else {
                return fail(format!("copy to {dst}"));
            };
            let Some(value) = self.values.get(src).cloned() else {
                return fail(format!("untracked copy {line}"));
            };
            self.argument_registers.insert(register, value);
            return Ok(());
        }
        if matches!(src, "$rsp" | "$sp") {
            self.values
                .insert(dst.to_string(), Value::pointer(Base::OutgoingArea(0)));
            return Ok(());
        }
        if let Some(register) = src.strip_prefix('$') {
            let register = canonical_register(register);
            let bytes = (0..16)
                .map(|j| (j, Some(Source::Register(register.clone(), j))))
                .collect();
            self.values.insert(dst.to_string(), Value::bytes(bytes));
            return Ok(());
        }
        let (name, subregister) = match src.split_once('.') {
            Some((name, sub)) => (name, Some(sub)),
            None => (src, None),
        };
        let mut value = self.values.get(name).cloned().unwrap_or_default();
        if let Some(subregister) = subregister {
            let width = match subregister {
                "dsub" => 8,
                "ssub" => 4,
                "hsub" => 2,
                "bsub" => 1,
                "sub_32" | "sub_32bit" => 4,
                "sub_16bit" => 2,
                "sub_8bit" => 1,
                "sub_xmm" => 16,
                other => return fail(format!("subreg {other}")),
            };
            if value.pointer.is_some() {
                return fail("subreg of pointer");
            }
            value.bytes.retain(|j, _| *j < width);
        }
        self.values.insert(dst.to_string(), value);
        Ok(())
    }

    /// A shift by whole bytes, which clang uses to pack small records.
    fn byte_shift(&self, op: &str, ops: &[&str]) -> Result<Option<Value>, HarnessFailure> {
        let shift = matches!(op, "SHL64ri" | "SHL32ri" | "SHR64ri" | "SHR32ri")
            || (op == "UBFMXri" && ops.get(2) == Some(&"63"))
            || (op == "UBFMWri" && ops.get(2) == Some(&"31"));
        if !shift {
            return Ok(None);
        }
        let bits = integer(operand(ops, 1)?)?;
        if bits % 8 != 0 {
            return fail("bit shift");
        }
        let width = if op.contains("64") || op.ends_with("Xri") {
            8
        } else {
            4
        };
        let delta = if op.starts_with("SHR") || op.starts_with("UBFM") {
            -bits / 8
        } else {
            bits / 8
        };
        let bytes = self
            .value(operand(ops, 0)?)?
            .bytes
            .iter()
            .filter(|(j, _)| (0..width).contains(&(**j + delta)))
            .map(|(j, s)| (j + delta, s.clone()))
            .collect();
        Ok(Some(Value::bytes(bytes)))
    }

    fn load(
        &mut self,
        dst: &str,
        op: &str,
        ops: &[&str],
        line: &str,
    ) -> Result<(), HarnessFailure> {
        let size = access_size(op)?;
        let (base, offset) = if op.starts_with("MOV") {
            if operand(ops, 0)? == "$rip" {
                let target = operand(ops, 3)?;
                let Some((name, offset)) = symbol(target) else {
                    return fail(format!("rip load {line}"));
                };
                if target.contains("target-flags(x86-gotpcrel)") {
                    self.values
                        .insert(dst.to_string(), Value::pointer(Base::Global(name, offset)));
                    return Ok(());
                }
                (Some(Base::Global(name, offset)), 0)
            } else {
                (self.base_of(operand(ops, 0)?), integer(operand(ops, 3)?)?)
            }
        } else {
            let immediate = integer(operand(ops, 1)?)?;
            let scale = if op.starts_with("LDUR") { 1 } else { size };
            (self.base_of(operand(ops, 0)?), immediate * scale)
        };
        let Some(base) = base else {
            return fail(format!("load base {line}"));
        };
        let value = self.read(&base, offset, size);
        self.values.insert(dst.to_string(), value);
        Ok(())
    }

    fn store(&mut self, op: &str, ops: &[&str], line: &str) -> Result<(), HarnessFailure> {
        let size = access_size(op)?;
        let (value, base, offset) = if op.starts_with("MOV") {
            let value = ops.get(5).and_then(|v| self.values.get(*v)).cloned();
            if operand(ops, 0)? == "$rip" {
                let base = symbol(operand(ops, 3)?).map(|(name, at)| Base::Global(name, at));
                (value, base, 0)
            } else {
                (
                    value,
                    self.base_of(operand(ops, 0)?),
                    integer(operand(ops, 3)?)?,
                )
            }
        } else {
            let value = self.values.get(operand(ops, 0)?).cloned();
            let immediate = integer(operand(ops, 2)?)?;
            let scale = if op.starts_with("STUR") { 1 } else { size };
            (value, self.base_of(operand(ops, 1)?), immediate * scale)
        };
        let (Some(value), Some(base)) = (value, base) else {
            return fail(format!("store {line}"));
        };
        self.write(&base, offset, size, &value);
        Ok(())
    }
}

fn is_extend_move(op: &str) -> bool {
    let Some(rest) = op
        .strip_prefix("MOVSX")
        .or_else(|| op.strip_prefix("MOVZX"))
    else {
        return false;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && rest[digits..].starts_with("rr")
}

/// Splits `%5:gpr64 = OP a, b` into destination, opcode and operands, with
/// implicit operands removed.
fn instruction(line: &str) -> Option<(Option<String>, String, Vec<String>)> {
    let (dst, rest) = match line.split_once(" = ") {
        Some((dst, rest)) if !dst.contains(' ') => {
            let dst = dst.split_once(':').map_or(dst, |(name, _)| name);
            (Some(dst.to_string()), rest)
        }
        _ => (None, line),
    };
    let mut parts = rest.trim().splitn(2, char::is_whitespace);
    let op = parts.next()?.to_string();
    let ops = parts
        .next()
        .map(|rest| {
            rest.split(',')
                .map(str::trim)
                .filter(|op| {
                    !op.is_empty()
                        && !op.starts_with("implicit ")
                        && !op.starts_with("implicit-def ")
                })
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some((dst, op, ops))
}

fn trace_caller(body: &[&str]) -> Result<CallState, HarnessFailure> {
    let mut tracer = Tracer {
        values: BTreeMap::new(),
        stack_objects: BTreeMap::new(),
        outgoing_area: BTreeMap::new(),
        argument_registers: BTreeMap::new(),
        globals_written: BTreeMap::new(),
    };
    let mut at_call = None;
    for raw in body {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with("bb.")
            || line.starts_with("successors")
            || line.starts_with("liveins")
        {
            continue;
        }
        let line = without_flags(line.split(" :: ").next().unwrap_or(line));
        let Some((dst, op, ops)) = instruction(&line) else {
            return fail(format!("unparsed {line}"));
        };
        let ops: Vec<&str> = ops.iter().map(String::as_str).collect();
        match op.as_str() {
            "ADJCALLSTACKDOWN64" | "ADJCALLSTACKDOWN" | "ADJCALLSTACKUP64" | "ADJCALLSTACKUP"
            | "RET64" | "RET_ReallyLR" => continue,
            "IMPLICIT_DEF" => {
                if let Some(dst) = dst {
                    tracer.values.insert(dst, Value::default());
                }
                continue;
            }
            "CALL64pcrel32" | "BL" => {
                if operand(&ops, 0)?.contains("memcpy") {
                    tracer.memcpy()?;
                    continue;
                }
                if at_call.is_some() {
                    return fail("second real call");
                }
                at_call = Some((
                    tracer.argument_registers.clone(),
                    tracer.outgoing_area.clone(),
                ));
                continue;
            }
            _ => {}
        }
        tracer.step(dst.as_deref(), &op, &ops, &line)?;
    }
    let Some((argument_registers, outgoing_area)) = at_call else {
        return fail("no call");
    };
    Ok(CallState {
        argument_registers,
        outgoing_area,
        stack_objects: tracer.stack_objects,
        globals_written: tracer.globals_written,
    })
}

/// Byte placements, in the location spelling `reg:x1@0`, `sp+8` or
/// `ind(reg:x8)@4`, of each parameter and of the result of caller `c<k>`.
pub(super) struct Placements {
    pub(super) parameters: Vec<BTreeMap<u32, String>>,
    pub(super) result: BTreeMap<u32, String>,
}

fn parameter_byte(source: &Option<Source>, k: usize) -> Option<(usize, u32)> {
    let Some(Source::Global(name, offset)) = source else {
        return None;
    };
    let index = name.strip_prefix(&format!("g{k}_"))?.parse().ok()?;
    Some((index, u32::try_from(*offset).ok()?))
}

pub(super) fn clang_placements(
    k: usize,
    body: &[&str],
    parameter_count: usize,
) -> Result<Placements, HarnessFailure> {
    let state = trace_caller(body)?;
    let mut parameters = vec![BTreeMap::new(); parameter_count];
    // Stack objects whose address clang passes, by where the address goes.
    let mut pointers: BTreeMap<u32, String> = BTreeMap::new();
    let place = |parameters: &mut Vec<BTreeMap<u32, String>>, (i, byte): (usize, u32), at| {
        if let Some(parameter) = parameters.get_mut(i) {
            parameter.insert(byte, at);
        }
    };
    for (register, value) in &state.argument_registers {
        if let Some(pointer) = &value.pointer {
            if let Base::StackObject(object, _) = pointer {
                pointers.insert(*object, format!("reg:{register}"));
            }
            continue;
        }
        for (j, source) in &value.bytes {
            if let Some(byte) = parameter_byte(source, k) {
                place(&mut parameters, byte, format!("reg:{register}@{j}"));
            }
        }
    }
    for (offset, source) in &state.outgoing_area {
        if let Some(Source::Pointer(Base::StackObject(object, _))) = source {
            pointers
                .entry(*object)
                .or_insert_with(|| format!("sp+{offset}"));
            continue;
        }
        if let Some(byte) = parameter_byte(source, k) {
            place(&mut parameters, byte, format!("sp+{offset}"));
        }
    }
    for (object, at) in &pointers {
        for (offset, source) in state.stack_objects.get(object).into_iter().flatten() {
            if let Some((i, byte)) = parameter_byte(source, k)
                && let Some(parameter) = parameters.get_mut(i)
            {
                parameter
                    .entry(byte)
                    .or_insert_with(|| format!("ind({at})@{offset}"));
            }
        }
    }
    let mut result = BTreeMap::new();
    let result_global = format!("r{k}");
    for ((global, offset), source) in &state.globals_written {
        let (true, Some(source), Ok(offset)) =
            (*global == result_global, source, u32::try_from(*offset))
        else {
            continue;
        };
        let at = match source {
            Source::Register(register, j) => {
                format!("reg:{}@{j}", register.as_deref().unwrap_or("?"))
            }
            Source::StackMemory(object, j) if pointers.contains_key(object) => {
                format!("ind({})@{j}", pointers[object])
            }
            other => format!("?{other:?}"),
        };
        result.insert(offset, at);
    }
    Ok(Placements { parameters, result })
}
