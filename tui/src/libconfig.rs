//! Subset libconfig parser and renderer used by the TUI.
//! Comments are accepted on read and dropped on write (v1).

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    List(Vec<Value>),
    Group(Vec<(String, Value)>),
}

impl Value {
    pub fn as_group(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Group(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) | Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn lookup<'a>(&'a self, key: &str) -> Option<&'a Value> {
        let group = self.as_group()?;
        group.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(v) => Some(*v),
            Value::Int(v) => Some(*v != 0),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            Value::Float(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            line: self.line,
            column: self.column,
            message: message.into(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.peek() {
            self.bump();
            if ch == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), ParseError> {
        loop {
            match self.bump() {
                None => return Err(self.error("unterminated block comment")),
                Some('*') if self.peek() == Some('/') => {
                    self.bump();
                    return Ok(());
                }
                Some(_) => {}
            }
        }
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(ch) if ch.is_whitespace() => {
                    self.bump();
                }
                Some('/') => {
                    let rest = &self.src[self.pos..];
                    if rest.starts_with("//") {
                        self.skip_line_comment();
                    } else if rest.starts_with("/*") {
                        self.bump();
                        self.bump();
                        self.skip_block_comment()?;
                    } else {
                        return Ok(());
                    }
                }
                Some('#') => self.skip_line_comment(),
                _ => return Ok(()),
            }
        }
    }

    fn skip_separators(&mut self) -> Result<(), ParseError> {
        loop {
            self.skip_ws_and_comments()?;
            match self.peek() {
                Some(';') | Some(',') => {
                    self.bump();
                }
                _ => return Ok(()),
            }
        }
    }

    fn parse_ident(&mut self) -> Result<String, ParseError> {
        self.skip_ws_and_comments()?;
        let start = self.pos;
        match self.peek() {
            Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {}
            _ => return Err(self.error("expected identifier")),
        }
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                self.bump();
            } else {
                break;
            }
        }
        Ok(self.src[start..self.pos].to_string())
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        if self.bump() != Some('"') {
            return Err(self.error("expected string"));
        }
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(self.error("unterminated string")),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('\\') => out.push('\\'),
                    Some('"') => out.push('"'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => return Err(self.error("unterminated string escape")),
                },
                Some(ch) => out.push(ch),
            }
        }
    }

    fn parse_number(&mut self) -> Result<Value, ParseError> {
        let start_line = self.line;
        let start_col = self.column;
        let start = self.pos;
        if self.peek() == Some('+') || self.peek() == Some('-') {
            self.bump();
        }
        let rest = &self.src[self.pos..];
        if rest.starts_with("0x") || rest.starts_with("0X") {
            self.bump();
            self.bump();
            let hex_start = self.pos;
            while matches!(self.peek(), Some(ch) if ch.is_ascii_hexdigit()) {
                self.bump();
            }
            let hex = &self.src[hex_start..self.pos];
            let value = i64::from_str_radix(hex, 16).map_err(|_| ParseError {
                line: start_line,
                column: start_col,
                message: "invalid hex integer".into(),
            })?;
            return Ok(Value::Int(value));
        }
        let mut is_float = false;
        if self.peek() == Some('.') {
            is_float = true;
            self.bump();
        }
        while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
            self.bump();
        }
        if self.peek() == Some('.') {
            is_float = true;
            self.bump();
            while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            is_float = true;
            self.bump();
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.bump();
            }
            while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
                self.bump();
            }
        }
        let token = &self.src[start..self.pos];
        if is_float {
            let value = token.parse::<f64>().map_err(|_| ParseError {
                line: start_line,
                column: start_col,
                message: format!("invalid float '{token}'"),
            })?;
            return Ok(Value::Float(value));
        }
        let value = token.parse::<i64>().map_err(|_| ParseError {
            line: start_line,
            column: start_col,
            message: format!("invalid integer '{token}'"),
        })?;
        Ok(Value::Int(value))
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        self.skip_ws_and_comments()?;
        match self.peek() {
            Some('{') => self.parse_group(),
            Some('(') => self.parse_collection(true),
            Some('[') => self.parse_collection(false),
            Some('"') => Ok(Value::String(self.parse_string()?)),
            Some(ch) if ch.is_ascii_digit() || ch == '+' || ch == '-' || ch == '.' => {
                self.parse_number()
            }
            Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {
                let ident = self.parse_ident()?;
                match ident.as_str() {
                    "true" | "True" | "TRUE" => Ok(Value::Bool(true)),
                    "false" | "False" | "FALSE" => Ok(Value::Bool(false)),
                    other => Err(self.error(format!("unexpected identifier '{other}' in value"))),
                }
            }
            Some(ch) => Err(self.error(format!("unexpected '{ch}'"))),
            None => Err(self.error("unexpected end of file")),
        }
    }

    fn parse_group(&mut self) -> Result<Value, ParseError> {
        if self.bump() != Some('{') {
            return Err(self.error("expected '{'"));
        }
        let mut items = Vec::new();
        loop {
            self.skip_ws_and_comments()?;
            if self.peek() == Some('}') {
                self.bump();
                return Ok(Value::Group(items));
            }
            if self.peek().is_none() {
                return Err(self.error("unterminated group"));
            }
            let key = self.parse_ident()?;
            self.skip_ws_and_comments()?;
            match self.peek() {
                Some('=') | Some(':') => {
                    self.bump();
                }
                _ => return Err(self.error("expected '=' after setting name")),
            }
            let value = self.parse_value()?;
            items.push((key, value));
            self.skip_separators()?;
        }
    }

    fn parse_collection(&mut self, is_list: bool) -> Result<Value, ParseError> {
        let open = if is_list { '(' } else { '[' };
        let close = if is_list { ')' } else { ']' };
        if self.bump() != Some(open) {
            return Err(self.error(format!("expected '{open}'")));
        }
        let mut items = Vec::new();
        loop {
            self.skip_ws_and_comments()?;
            if self.peek() == Some(close) {
                self.bump();
                return Ok(if is_list {
                    Value::List(items)
                } else {
                    Value::Array(items)
                });
            }
            if self.peek().is_none() {
                return Err(self.error("unterminated collection"));
            }
            items.push(self.parse_value()?);
            self.skip_separators()?;
        }
    }

    fn parse_document(&mut self) -> Result<Value, ParseError> {
        let mut items = Vec::new();
        loop {
            self.skip_ws_and_comments()?;
            if self.peek().is_none() {
                break;
            }
            let key = self.parse_ident()?;
            self.skip_ws_and_comments()?;
            match self.peek() {
                Some('=') | Some(':') => {
                    self.bump();
                }
                _ => return Err(self.error("expected '=' after setting name")),
            }
            let value = self.parse_value()?;
            items.push((key, value));
            self.skip_separators()?;
        }
        Ok(Value::Group(items))
    }
}

pub fn parse(src: &str) -> Result<Value, ParseError> {
    Parser::new(src).parse_document()
}

fn render_value(value: &Value, indent: usize, out: &mut String) {
    match value {
        Value::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
        Value::Int(v) => out.push_str(&v.to_string()),
        Value::Float(v) => {
            if v.fract() == 0.0 {
                out.push_str(&format!("{v:.1}"));
            } else {
                out.push_str(&format!("{v}"));
            }
        }
        Value::String(v) => {
            out.push('"');
            for ch in v.chars() {
                match ch {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    other => out.push(other),
                }
            }
            out.push('"');
        }
        Value::Array(items) => render_collection(items, '[', ']', indent, out),
        Value::List(items) => render_collection(items, '(', ')', indent, out),
        Value::Group(items) => render_group(items, indent, out),
    }
}

fn pad(indent: usize, out: &mut String) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

fn render_collection(items: &[Value], open: char, close: char, indent: usize, out: &mut String) {
    out.push(open);
    if items.is_empty() {
        out.push(close);
        return;
    }
    out.push('\n');
    for (index, item) in items.iter().enumerate() {
        pad(indent + 1, out);
        render_value(item, indent + 1, out);
        if index + 1 != items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    pad(indent, out);
    out.push(close);
}

fn render_group(items: &[(String, Value)], indent: usize, out: &mut String) {
    out.push('{');
    if items.is_empty() {
        out.push('}');
        return;
    }
    out.push('\n');
    for (key, value) in items {
        pad(indent + 1, out);
        out.push_str(key);
        out.push_str(" = ");
        render_value(value, indent + 1, out);
        out.push(';');
        out.push('\n');
    }
    pad(indent, out);
    out.push('}');
}

pub fn render(root: &Value) -> String {
    let mut out = String::new();
    let Some(items) = root.as_group() else {
        render_value(root, 0, &mut out);
        out.push('\n');
        return out;
    };
    for (key, value) in items {
        out.push_str(key);
        out.push_str(" = ");
        render_value(value, 0, &mut out);
        out.push(';');
        out.push('\n');
    }
    out
}

pub fn group_get<'a>(items: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    items.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
}

pub fn group_set(items: &mut Vec<(String, Value)>, key: &str, value: Value) {
    if let Some(existing) = items.iter_mut().find(|(k, _)| k == key) {
        existing.1 = value;
        return;
    }
    items.push((key.to_string(), value));
}

pub fn group_remove(items: &mut Vec<(String, Value)>, key: &str) {
    items.retain(|(k, _)| k != key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_and_missing_semicolons() {
        let src = r#"
            configs = (
                {
                    sim = "default"
                    car = "default";
                    devices = (
                        {
                            device = "Sound"
                            effect = "Gear"
                            volume = 100;
                        }
                    );
                }
            );
        "#;
        let root = parse(src).expect("parse");
        let configs = root.lookup("configs").unwrap().as_list().unwrap();
        assert_eq!(configs.len(), 1);
        let sim = configs[0].lookup("sim").unwrap().as_str().unwrap();
        assert_eq!(sim, "default");
        let devices = configs[0].lookup("devices").unwrap().as_list().unwrap();
        assert_eq!(devices[0].lookup("effect").unwrap().as_str().unwrap(), "Gear");
    }

    #[test]
    fn round_trip_group() {
        let src = r#"a = 1; b = "x"; c = true;"#;
        let root = parse(src).unwrap();
        let rendered = render(&root);
        let again = parse(&rendered).unwrap();
        assert_eq!(root, again);
    }
}
