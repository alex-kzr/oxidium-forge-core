use super::lexer::{tokenize, Token};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Var(Vec<String>), // path: ["order", "status"] for order.status
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    UnaryMinus(Box<Expr>),
    Not(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Expression parse error: {}", self.0)
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect_end(&self) -> Result<(), ParseError> {
        if self.pos < self.tokens.len() {
            Err(ParseError(format!(
                "Unexpected token at position {}: {:?}",
                self.pos,
                self.tokens[self.pos]
            )))
        } else {
            Ok(())
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinOp(Box::new(left), BinOp::Or, Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.advance();
            let right = self.parse_not()?;
            left = Expr::BinOp(Box::new(left), BinOp::And, Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let inner = self.parse_not()?;
            Ok(Expr::Not(Box::new(inner)))
        } else {
            self.parse_compare()
        }
    }

    fn parse_compare(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_add()?;
        let op = match self.peek() {
            Some(Token::Eq) => BinOp::Eq,
            Some(Token::Ne) => BinOp::Ne,
            Some(Token::Lt) => BinOp::Lt,
            Some(Token::Le) => BinOp::Le,
            Some(Token::Gt) => BinOp::Gt,
            Some(Token::Ge) => BinOp::Ge,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_add()?;
        Ok(Expr::BinOp(Box::new(left), op, Box::new(right)))
    }

    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.advance();
            let inner = self.parse_unary()?;
            Ok(Expr::UnaryMinus(Box::new(inner)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().cloned() {
            Some(Token::Number(n)) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Some(Token::Str(s)) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Some(Token::Null) => {
                self.advance();
                Ok(Expr::Null)
            }
            Some(Token::LParen) => {
                self.advance();
                let inner = self.parse_expr()?;
                if !matches!(self.peek(), Some(Token::RParen)) {
                    return Err(ParseError("Expected closing ')'".into()));
                }
                self.advance();
                Ok(inner)
            }
            Some(Token::Ident(name)) => {
                self.advance();
                let mut path = vec![name];
                while matches!(self.peek(), Some(Token::Dot)) {
                    self.advance();
                    match self.peek().cloned() {
                        Some(Token::Ident(seg)) => {
                            self.advance();
                            path.push(seg);
                        }
                        other => {
                            return Err(ParseError(format!(
                                "Expected identifier after '.', got {:?}",
                                other
                            )));
                        }
                    }
                }
                Ok(Expr::Var(path))
            }
            other => Err(ParseError(format!("Unexpected token: {:?}", other))),
        }
    }
}

/// Parse a FEEL-subset expression string.
/// Strips a leading `=` prefix (Camunda convention) before parsing.
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    let trimmed = input.trim();
    let expr_str = trimmed.strip_prefix('=').unwrap_or(trimmed).trim();

    if expr_str.is_empty() {
        return Err(ParseError("Empty expression".into()));
    }

    let tokens =
        tokenize(expr_str).map_err(|e| ParseError(format!("Lex error: {}", e.0)))?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr()?;
    parser.expect_end()?;
    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_number_comparison() {
        let e = parse("amount > 100").unwrap();
        assert!(matches!(e, Expr::BinOp(_, BinOp::Gt, _)));
    }

    #[test]
    fn parse_with_eq_prefix() {
        let e = parse("= amount > 100").unwrap();
        assert!(matches!(e, Expr::BinOp(_, BinOp::Gt, _)));
    }

    #[test]
    fn parse_string_equality() {
        let e = parse(r#"status = "active""#).unwrap();
        assert!(matches!(e, Expr::BinOp(_, BinOp::Eq, _)));
    }

    #[test]
    fn parse_and_or() {
        let e = parse("a > 1 and b < 2 or c = true").unwrap();
        // or has lowest precedence, so: (a>1 and b<2) or (c=true)
        assert!(matches!(e, Expr::BinOp(_, BinOp::Or, _)));
    }

    #[test]
    fn parse_member_access() {
        let e = parse("order.status").unwrap();
        assert_eq!(e, Expr::Var(vec!["order".into(), "status".into()]));
    }

    #[test]
    fn empty_expression_errors() {
        assert!(parse("").is_err());
        assert!(parse("=").is_err());
    }
}
