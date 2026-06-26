#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Str(String),
    Ident(String),
    True,
    False,
    Null,
    Eq,     // =
    Ne,     // !=
    Lt,     // <
    Le,     // <=
    Gt,     // >
    Ge,     // >=
    And,    // and
    Or,     // or
    Not,    // not
    Plus,   // +
    Minus,  // -
    Star,   // *
    Slash,  // /
    LParen, // (
    RParen, // )
    Dot,    // .
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError(pub String);

pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut tokens = Vec::new();

    while pos < chars.len() {
        match chars[pos] {
            ' ' | '\t' | '\r' | '\n' => {
                pos += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                pos += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                pos += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                pos += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                pos += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                pos += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                pos += 1;
            }
            '.' => {
                tokens.push(Token::Dot);
                pos += 1;
            }
            '=' => {
                tokens.push(Token::Eq);
                pos += 1;
            }
            '!' if pos + 1 < chars.len() && chars[pos + 1] == '=' => {
                tokens.push(Token::Ne);
                pos += 2;
            }
            '<' if pos + 1 < chars.len() && chars[pos + 1] == '=' => {
                tokens.push(Token::Le);
                pos += 2;
            }
            '<' => {
                tokens.push(Token::Lt);
                pos += 1;
            }
            '>' if pos + 1 < chars.len() && chars[pos + 1] == '=' => {
                tokens.push(Token::Ge);
                pos += 2;
            }
            '>' => {
                tokens.push(Token::Gt);
                pos += 1;
            }
            '"' => {
                pos += 1;
                let mut s = String::new();
                while pos < chars.len() && chars[pos] != '"' {
                    if chars[pos] == '\\' && pos + 1 < chars.len() {
                        match chars[pos + 1] {
                            '"' => {
                                s.push('"');
                                pos += 2;
                            }
                            '\\' => {
                                s.push('\\');
                                pos += 2;
                            }
                            'n' => {
                                s.push('\n');
                                pos += 2;
                            }
                            'r' => {
                                s.push('\r');
                                pos += 2;
                            }
                            't' => {
                                s.push('\t');
                                pos += 2;
                            }
                            c => {
                                s.push(c);
                                pos += 2;
                            }
                        }
                    } else {
                        s.push(chars[pos]);
                        pos += 1;
                    }
                }
                if pos >= chars.len() {
                    return Err(LexError("Unterminated string literal".into()));
                }
                pos += 1; // closing "
                tokens.push(Token::Str(s));
            }
            c if c.is_ascii_digit() => {
                let start = pos;
                while pos < chars.len() && (chars[pos].is_ascii_digit() || chars[pos] == '.') {
                    pos += 1;
                }
                let s: String = chars[start..pos].iter().collect();
                let n: f64 = s
                    .parse()
                    .map_err(|_| LexError(format!("Invalid number: {s}")))?;
                tokens.push(Token::Number(n));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = pos;
                while pos < chars.len()
                    && (chars[pos].is_alphanumeric() || chars[pos] == '_')
                {
                    pos += 1;
                }
                let word: String = chars[start..pos].iter().collect();
                let tok = match word.as_str() {
                    "true" => Token::True,
                    "false" => Token::False,
                    "null" => Token::Null,
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    _ => Token::Ident(word),
                };
                tokens.push(tok);
            }
            c => {
                return Err(LexError(format!("Unexpected character: '{c}'")));
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_comparison() {
        let toks = tokenize("amount > 100").unwrap();
        assert_eq!(toks, vec![Token::Ident("amount".into()), Token::Gt, Token::Number(100.0)]);
    }

    #[test]
    fn lex_string_eq() {
        let toks = tokenize(r#"status = "active""#).unwrap();
        assert_eq!(
            toks,
            vec![Token::Ident("status".into()), Token::Eq, Token::Str("active".into())]
        );
    }

    #[test]
    fn lex_keywords() {
        let toks = tokenize("true and false or null").unwrap();
        assert_eq!(
            toks,
            vec![Token::True, Token::And, Token::False, Token::Or, Token::Null]
        );
    }
}
