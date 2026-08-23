use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JsonValue {
    Object(Vec<(String, JsonValue)>),
    Array(Vec<JsonValue>),
    String(String),
    Number(u64),
    Bool,
    Null,
}

impl JsonValue {
    pub(crate) fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self {
            JsonValue::Object(fields) => Some(fields),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(values) => Some(values),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JsonParseError {
    InvalidJson { message: String },
}

pub(crate) struct JsonParser<'a> {
    input: &'a str,
    index: usize,
}

impl<'a> JsonParser<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    pub(crate) fn parse(mut self) -> Result<JsonValue, JsonParseError> {
        self.skip_whitespace();
        let value = self.parse_value()?;
        self.skip_whitespace();
        if self.index != self.input.len() {
            return self.invalid_json("trailing characters after JSON value");
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonParseError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'0'..=b'9') => self.parse_number().map(JsonValue::Number),
            Some(b't') => {
                self.consume_literal("true")?;
                Ok(JsonValue::Bool)
            }
            Some(b'f') => {
                self.consume_literal("false")?;
                Ok(JsonValue::Bool)
            }
            Some(b'n') => {
                self.consume_literal("null")?;
                Ok(JsonValue::Null)
            }
            Some(_) => self.invalid_json("unexpected character while parsing JSON value"),
            None => self.invalid_json("unexpected end of JSON input"),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonParseError> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();
        let mut fields = Vec::new();
        let mut names = BTreeSet::new();
        if self.consume_byte(b'}') {
            return Ok(JsonValue::Object(fields));
        }
        loop {
            self.skip_whitespace();
            let name = self.parse_string()?;
            if !names.insert(name.clone()) {
                return Err(JsonParseError::InvalidJson {
                    message: format!("duplicate JSON object field `{name}`"),
                });
            }
            self.skip_whitespace();
            self.expect_byte(b':')?;
            let value = self.parse_value()?;
            fields.push((name, value));
            self.skip_whitespace();
            if self.consume_byte(b'}') {
                break;
            }
            self.expect_byte(b',')?;
        }
        Ok(JsonValue::Object(fields))
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonParseError> {
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume_byte(b']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            if self.consume_byte(b']') {
                break;
            }
            self.expect_byte(b',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_string(&mut self) -> Result<String, JsonParseError> {
        self.expect_byte(b'"')?;
        let mut value = String::new();
        while let Some(byte) = self.next_byte() {
            match byte {
                b'"' => return Ok(value),
                b'\\' => value.push(self.parse_escape()?),
                0x00..=0x1f => {
                    return self.invalid_json("unescaped control character in JSON string");
                }
                _ => {
                    let ch = self.input[self.index - 1..]
                        .chars()
                        .next()
                        .expect("byte index points at a character");
                    value.push(ch);
                    self.index = self.index - 1 + ch.len_utf8();
                }
            }
        }
        self.invalid_json("unterminated JSON string")
    }

    fn parse_escape(&mut self) -> Result<char, JsonParseError> {
        let Some(byte) = self.next_byte() else {
            return self.invalid_json("unterminated JSON escape");
        };
        match byte {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{0008}'),
            b'f' => Ok('\u{000c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => self.parse_unicode_escape(),
            _ => self.invalid_json("invalid JSON escape"),
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, JsonParseError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let Some(byte) = self.next_byte() else {
                return self.invalid_json("unterminated unicode escape");
            };
            let Some(digit) = (byte as char).to_digit(16) else {
                return self.invalid_json("invalid unicode escape digit");
            };
            value = (value << 4) | digit;
        }
        char::from_u32(value).ok_or_else(|| JsonParseError::InvalidJson {
            message: "invalid unicode scalar value".to_owned(),
        })
    }

    fn parse_number(&mut self) -> Result<u64, JsonParseError> {
        let start = self.index;
        if self.consume_byte(b'0') {
            if matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.invalid_json("JSON numbers cannot have leading zeroes");
            }
        } else {
            self.expect_digit()?;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return self.invalid_json("Omega package JSON accepts integer numbers only");
        }
        self.input[start..self.index]
            .parse::<u64>()
            .map_err(|error| JsonParseError::InvalidJson {
                message: format!("invalid JSON number: {error}"),
            })
    }

    fn consume_literal(&mut self, literal: &str) -> Result<(), JsonParseError> {
        if self.input[self.index..].starts_with(literal) {
            self.index += literal.len();
            Ok(())
        } else {
            self.invalid_json("invalid JSON literal")
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.index += 1;
        }
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), JsonParseError> {
        if self.consume_byte(expected) {
            Ok(())
        } else {
            self.invalid_json(&format!("expected `{}`", expected as char))
        }
    }

    fn expect_digit(&mut self) -> Result<(), JsonParseError> {
        match self.peek() {
            Some(b'1'..=b'9') => {
                self.index += 1;
                Ok(())
            }
            _ => self.invalid_json("expected digit"),
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn next_byte(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        Some(byte)
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.index).copied()
    }

    fn invalid_json<T>(&self, message: &str) -> Result<T, JsonParseError> {
        Err(JsonParseError::InvalidJson {
            message: format!("{message} at byte {}", self.index),
        })
    }
}
