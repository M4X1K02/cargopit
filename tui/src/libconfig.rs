//! Subset of libconfig used by monocoque.config: groups, arrays, scalars, comments.

use anyhow::{bail, Context, Result};
use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Group(Vec<(String, Value)>),
}

impl Value {
    pub fn as_group(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Group(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn lookup<'a>(&'a self, key: &str) -> Option<&'a Value> {
        self.as_group()?
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    #[cfg(test)]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn parse_file(&mut self) -> Result<Vec<(String, Value)>> {
        let mut entries = Vec::new();
        self.skip_trivia();
        while !self.is_eof() {
            let name = self.parse_ident()?;
            self.skip_trivia();
            self.expect_char('=')?;
            self.skip_trivia();
            let value = self.parse_value()?;
            self.skip_statement_end();
            entries.push((name, value));
            self.skip_trivia();
        }
        Ok(entries)
    }

    fn parse_value(&mut self) -> Result<Value> {
        self.skip_trivia();
        let ch = self.peek().context("unexpected end of file in value")?;
        if ch == '"' {
            return Ok(Value::String(self.parse_string()?));
        }
        if ch == '{' {
            return Ok(Value::Group(self.parse_group()?));
        }
        if ch == '(' {
            return Ok(Value::Array(self.parse_array()?));
        }
        if ch.is_ascii_digit() || ch == '-' || ch == '.' {
            return self.parse_number();
        }
        let ident = self.parse_ident()?;
        match ident.as_str() {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            other => Ok(Value::String(other.to_string())),
        }
    }

    fn parse_group(&mut self) -> Result<Vec<(String, Value)>> {
        self.expect_char('{')?;
        let mut entries = Vec::new();
        loop {
            self.skip_trivia();
            if self.peek() == Some('}') {
                self.pos += 1;
                break;
            }
            if self.is_eof() {
                bail!("unclosed group");
            }
            let name = self.parse_ident()?;
            self.skip_trivia();
            self.expect_char('=')?;
            self.skip_trivia();
            let value = self.parse_value()?;
            self.skip_statement_end();
            entries.push((name, value));
        }
        Ok(entries)
    }

    fn parse_array(&mut self) -> Result<Vec<Value>> {
        self.expect_char('(')?;
        let mut items = Vec::new();
        loop {
            self.skip_trivia();
            if self.peek() == Some(')') {
                self.pos += 1;
                break;
            }
            if self.is_eof() {
                bail!("unclosed array");
            }
            items.push(self.parse_value()?);
            self.skip_trivia();
            if self.peek() == Some(',') {
                self.pos += 1;
            }
        }
        Ok(items)
    }

    fn parse_string(&mut self) -> Result<String> {
        self.expect_char('"')?;
        let mut out = String::new();
        while let Some(ch) = self.next_char() {
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let escaped = self.next_char().context("unterminated string escape")?;
                    out.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        other => other,
                    });
                }
                other => out.push(other),
            }
        }
        bail!("unterminated string")
    }

    fn parse_number(&mut self) -> Result<Value> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
            self.pos += 1;
        }
        let mut is_float = false;
        if self.peek() == Some('.') {
            is_float = true;
            self.pos += 1;
            while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let token = &self.src[start..self.pos];
        if is_float {
            let value: f64 = token
                .parse()
                .with_context(|| format!("invalid float {token}"))?;
            Ok(Value::Float(value))
        } else {
            let value: i64 = token
                .parse()
                .with_context(|| format!("invalid int {token}"))?;
            Ok(Value::Int(value))
        }
    }

    fn parse_ident(&mut self) -> Result<String> {
        self.skip_trivia();
        let start = self.pos;
        let first = self.peek().context("expected identifier")?;
        if !first.is_ascii_alphabetic() && first != '_' {
            bail!("expected identifier, found {:?}", first);
        }
        self.pos += 1;
        while matches!(self.peek(), Some(ch) if ch.is_ascii_alphanumeric() || ch == '_') {
            self.pos += 1;
        }
        Ok(self.src[start..self.pos].to_string())
    }

    fn skip_statement_end(&mut self) {
        self.skip_trivia();
        if self.peek() == Some(';') || self.peek() == Some(',') {
            self.pos += 1;
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            self.skip_ws();
            if self.src[self.pos..].starts_with("//") {
                if let Some(offset) = self.src[self.pos..].find('\n') {
                    self.pos += offset + 1;
                } else {
                    self.pos = self.src.len();
                }
                continue;
            }
            if self.src[self.pos..].starts_with("/*") {
                if let Some(offset) = self.src[self.pos + 2..].find("*/") {
                    self.pos += offset + 4;
                } else {
                    self.pos = self.src.len();
                }
                continue;
            }
            break;
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        let found = self
            .next_char()
            .with_context(|| format!("expected {expected}"))?;
        if found != expected {
            bail!("expected {expected}, found {found}");
        }
        Ok(())
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.src.len()
    }
}

pub fn parse(src: &str) -> Result<Vec<(String, Value)>> {
    Parser::new(src).parse_file()
}

pub fn render(entries: &[(String, Value)]) -> String {
    let mut out = String::new();
    for (name, value) in entries {
        let _ = write_assignment(&mut out, name, value, 0);
        out.push('\n');
    }
    out
}

fn write_assignment(
    out: &mut String,
    name: &str,
    value: &Value,
    indent: usize,
) -> Result<(), std::fmt::Error> {
    write_indent(out, indent)?;
    write!(out, "{name} = ")?;
    write_value(out, value, indent)?;
    out.push_str(";\n");
    Ok(())
}

fn write_value(out: &mut String, value: &Value, indent: usize) -> Result<(), std::fmt::Error> {
    match value {
        Value::Bool(flag) => write!(out, "{flag}"),
        Value::Int(number) => write!(out, "{number}"),
        Value::Float(number) => {
            if number.fract() == 0.0 {
                write!(out, "{number:.1}")
            } else {
                write!(out, "{number}")
            }
        }
        Value::String(text) => write!(out, "\"{}\"", escape_string(text)),
        Value::Array(items) => {
            out.push_str("(\n");
            for (index, item) in items.iter().enumerate() {
                write_indent(out, indent + 1)?;
                write_value(out, item, indent + 1)?;
                if index + 1 != items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            write_indent(out, indent)?;
            out.push(')');
            Ok(())
        }
        Value::Group(entries) => {
            out.push_str("{\n");
            for (name, child) in entries {
                write_assignment(out, name, child, indent + 1)?;
            }
            write_indent(out, indent)?;
            out.push('}');
            Ok(())
        }
    }
}

fn write_indent(out: &mut String, indent: usize) -> Result<(), std::fmt::Error> {
    for _ in 0..indent {
        out.push_str("    ");
    }
    Ok(())
}

fn escape_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_group_and_array() {
        let src = r#"
            configs = (
                {
                    sim = "default";
                    car = "default";
                }
            );
        "#;
        let root = parse(src).unwrap();
        let configs = root[0].1.as_array().unwrap();
        let sim = configs[0].lookup("sim").unwrap().as_str().unwrap();
        assert_eq!(sim, "default");
    }

    #[test]
    fn accepts_missing_semicolons() {
        let src = r#"
            configs = (
                {
                    sim = "ac"
                    car = "default"
                }
            )
        "#;
        let root = parse(src).unwrap();
        assert_eq!(root[0].1.as_array().unwrap().len(), 1);
    }
}
