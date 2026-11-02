//! Tokenizer. Numbers may carry a unit suffix (`92C`, `9bar`, `250ms`, `2ml/s`).

use anyhow::{Result, bail};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Num(f64, Option<String>),
    Str(String),
    Sym(&'static str),
}

impl std::fmt::Display for Tok {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tok::Ident(s) => write!(f, "`{s}`"),
            Tok::Num(n, Some(u)) => write!(f, "`{n}{u}`"),
            Tok::Num(n, None) => write!(f, "`{n}`"),
            Tok::Str(s) => write!(f, "\"{s}\""),
            Tok::Sym(s) => write!(f, "`{s}`"),
        }
    }
}

const SYMS: &[&str] = &[
    "==", "!=", "<=", ">=", "<", ">", "=", ":", ";", ",", "(", ")", "+", "-",
];

pub fn lex(line: &str) -> Result<Vec<Tok>> {
    let mut out = Vec::new();
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c == '#' {
            break;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '"' {
            let start = i + 1;
            let mut j = start;
            while j < b.len() && b[j] != b'"' {
                j += 1;
            }
            if j >= b.len() {
                bail!("unterminated string");
            }
            out.push(Tok::Str(line[start..j].to_string()));
            i = j + 1;
            continue;
        }
        if c.is_ascii_digit()
            || (c == '.' && i + 1 < b.len() && (b[i + 1] as char).is_ascii_digit())
        {
            let start = i;
            while i < b.len() && ((b[i] as char).is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            // exponent
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') && i + 1 < b.len() {
                let n1 = b[i + 1] as char;
                if n1.is_ascii_digit()
                    || ((n1 == '-' || n1 == '+')
                        && i + 2 < b.len()
                        && (b[i + 2] as char).is_ascii_digit())
                {
                    i += 2;
                    while i < b.len() && (b[i] as char).is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let n: f64 = line[start..i]
                .parse()
                .map_err(|_| anyhow::anyhow!("bad number `{}`", &line[start..i]))?;
            let ustart = i;
            while i < b.len()
                && ((b[i] as char).is_ascii_alphabetic() || b[i] == b'/' || b[i] == b'%')
            {
                i += 1;
            }
            let unit = (i > ustart).then(|| line[ustart..i].to_string());
            out.push(Tok::Num(n, unit));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < b.len()
                && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.')
            {
                i += 1;
            }
            out.push(Tok::Ident(line[start..i].to_string()));
            continue;
        }
        if let Some(s) = SYMS.iter().find(|s| line[i..].starts_with(**s)) {
            out.push(Tok::Sym(s));
            i += s.len();
            continue;
        }
        bail!("unexpected character `{c}`");
    }
    Ok(out)
}

/// Convert a number with a unit to seconds.
pub fn to_seconds(n: f64, unit: Option<&str>) -> Result<f64> {
    Ok(match unit {
        None | Some("s") | Some("sec") => n,
        Some("ms") => n / 1000.0,
        Some("m") | Some("min") => n * 60.0,
        Some("h") => n * 3600.0,
        Some(u) => bail!("`{u}` is not a time unit (use ms, s, min, h)"),
    })
}

/// Units accepted on plain values. They are documentation only: values are used
/// in the signal's native unit (C, bar, g, ml, ml/s, V, W, s).
pub fn check_value_unit(unit: Option<&str>) -> Result<f64> {
    Ok(match unit {
        None => 1.0,
        Some("C" | "bar" | "g" | "ml" | "ml/s" | "g/s" | "V" | "W" | "s" | "Hz" | "hz" | "kJ") => {
            1.0
        }
        Some("ms") => 0.001,
        Some("min") => 60.0,
        Some("%") => 0.01,
        Some(u) => bail!("unknown unit `{u}`"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lexes() {
        let t = lex("at warm+12s: set pump.wear = 0.6 # comment").unwrap();
        assert_eq!(
            t,
            vec![
                Tok::Ident("at".into()),
                Tok::Ident("warm".into()),
                Tok::Sym("+"),
                Tok::Num(12.0, Some("s".into())),
                Tok::Sym(":"),
                Tok::Ident("set".into()),
                Tok::Ident("pump.wear".into()),
                Tok::Sym("="),
                Tok::Num(0.6, None),
            ]
        );
        assert_eq!(
            lex("x >= 2ml/s").unwrap()[2],
            Tok::Num(2.0, Some("ml/s".into()))
        );
    }
}
