use crate::variables::VariableMap;
use std::collections::HashMap;

/// Trait for resolving variable values numerically within formulas.
pub trait VariableLookup {
    fn get_variable_value(&self, name: &str) -> Option<f64>;
}

impl VariableLookup for VariableMap {
    fn get_variable_value(&self, name: &str) -> Option<f64> {
        self.get_num(name)
    }
}

impl VariableLookup for HashMap<String, f64> {
    fn get_variable_value(&self, name: &str) -> Option<f64> {
        let lower = name.to_ascii_lowercase();
        self.get(&lower).copied().or_else(|| self.get(name).copied())
    }
}

impl VariableLookup for HashMap<String, String> {
    fn get_variable_value(&self, name: &str) -> Option<f64> {
        let lower = name.to_ascii_lowercase();
        self.get(&lower)
            .or_else(|| self.get(name))
            .and_then(|s| s.trim().parse::<f64>().ok())
    }
}

impl<T: VariableLookup + ?Sized> VariableLookup for &T {
    fn get_variable_value(&self, name: &str) -> Option<f64> {
        (*self).get_variable_value(name)
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum FormulaError {
    #[error("Formula syntax error: {0}")]
    SyntaxError(String),
    #[error("Unknown variable: {0}")]
    UnknownVariable(String),
    #[error("Unknown function: {0}")]
    UnknownFunction(String),
    #[error("Invalid argument count for {func}: expected {expected}, got {got}")]
    InvalidArguments {
        func: String,
        expected: usize,
        got: usize,
    },
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Empty expression")]
    EmptyExpression,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Var(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Power,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Question,
    Colon,
    LParen,
    RParen,
    Comma,
    Eof,
}

fn tokenize(expr: &str) -> Result<Vec<Token>, FormulaError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    let len = chars.len();

    while i < len {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Floating point numbers
        if c.is_ascii_digit() || (c == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            let start = i;
            let mut seen_dot = c == '.';
            i += 1;
            while i < len {
                let ch = chars[i];
                if ch.is_ascii_digit() {
                    i += 1;
                } else if ch == '.' && !seen_dot {
                    seen_dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            let s: String = chars[start..i].iter().collect();
            let num = s
                .parse::<f64>()
                .map_err(|_| FormulaError::SyntaxError(format!("Invalid number: {}", s)))?;
            tokens.push(Token::Number(num));
            continue;
        }

        // #VarName#
        if c == '#' {
            let start = i + 1;
            let mut end = start;
            while end < len && chars[end] != '#' {
                end += 1;
            }
            if end >= len {
                return Err(FormulaError::SyntaxError(
                    "Unclosed variable macro '#' in formula".to_string(),
                ));
            }
            let var_name: String = chars[start..end].iter().collect();
            tokens.push(Token::Var(var_name));
            i = end + 1;
            continue;
        }

        // Identifiers: functions or variables
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            tokens.push(Token::Ident(ident));
            continue;
        }

        // Operators
        match c {
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => {
                if i + 1 < len && chars[i + 1] == '*' {
                    tokens.push(Token::Power);
                    i += 1;
                } else {
                    tokens.push(Token::Star);
                }
            }
            '/' => tokens.push(Token::Slash),
            '%' => tokens.push(Token::Percent),
            '^' => tokens.push(Token::Power),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ',' => tokens.push(Token::Comma),
            '?' => tokens.push(Token::Question),
            ':' => tokens.push(Token::Colon),
            '<' => {
                if i + 1 < len && chars[i + 1] == '>' {
                    tokens.push(Token::Ne);
                    i += 1;
                } else if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Le);
                    i += 1;
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Ge);
                    i += 1;
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '=' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    i += 1;
                }
                tokens.push(Token::Eq);
            }
            '!' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Ne);
                    i += 1;
                } else {
                    tokens.push(Token::Not);
                }
            }
            '&' => {
                if i + 1 < len && chars[i + 1] == '&' {
                    i += 1;
                }
                tokens.push(Token::And);
            }
            '|' => {
                if i + 1 < len && chars[i + 1] == '|' {
                    i += 1;
                }
                tokens.push(Token::Or);
            }
            _ => {
                return Err(FormulaError::SyntaxError(format!(
                    "Unexpected character: '{}'",
                    c
                )));
            }
        }
        i += 1;
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfixOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Ternary,
}

struct Parser<'a, V: VariableLookup> {
    tokens: Vec<Token>,
    pos: usize,
    vars: &'a V,
}

impl<'a, V: VariableLookup> Parser<'a, V> {
    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn next(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            tok
        } else {
            Token::Eof
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), FormulaError> {
        let tok = self.next();
        if tok == expected {
            Ok(())
        } else {
            Err(FormulaError::SyntaxError(format!(
                "Expected {:?}, got {:?}",
                expected, tok
            )))
        }
    }

    fn parse_expression(&mut self, min_bp: u8) -> Result<f64, FormulaError> {
        let tok = self.next();
        let mut lhs = match tok {
            Token::Number(n) => n,
            Token::Var(name) => self
                .vars
                .get_variable_value(&name)
                .ok_or_else(|| FormulaError::UnknownVariable(name))?,
            Token::Ident(ident) => {
                if *self.peek() == Token::LParen {
                    self.next(); // consume LParen
                    let mut args = Vec::new();
                    if *self.peek() != Token::RParen {
                        loop {
                            let arg = self.parse_expression(0)?;
                            args.push(arg);
                            if *self.peek() == Token::Comma {
                                self.next();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    eval_function(&ident, &args)?
                } else {
                    self.vars
                        .get_variable_value(&ident)
                        .ok_or_else(|| FormulaError::UnknownVariable(ident))?
                }
            }
            Token::Plus => self.parse_expression(8)?,
            Token::Minus => -self.parse_expression(8)?,
            Token::Not => {
                let val = self.parse_expression(8)?;
                if val == 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Token::LParen => {
                let inner = self.parse_expression(0)?;
                self.expect(Token::RParen)?;
                inner
            }
            Token::Eof => return Err(FormulaError::EmptyExpression),
            other => {
                return Err(FormulaError::SyntaxError(format!(
                    "Unexpected token: {:?}",
                    other
                )))
            }
        };

        loop {
            let (left_bp, right_bp, op) = match self.peek() {
                Token::Question => (1, 1, InfixOp::Ternary),
                Token::Or => (2, 3, InfixOp::Or),
                Token::And => (3, 4, InfixOp::And),
                Token::Eq => (4, 5, InfixOp::Eq),
                Token::Ne => (4, 5, InfixOp::Ne),
                Token::Lt => (4, 5, InfixOp::Lt),
                Token::Le => (4, 5, InfixOp::Le),
                Token::Gt => (4, 5, InfixOp::Gt),
                Token::Ge => (4, 5, InfixOp::Ge),
                Token::Plus => (5, 6, InfixOp::Add),
                Token::Minus => (5, 6, InfixOp::Sub),
                Token::Star => (6, 7, InfixOp::Mul),
                Token::Slash => (6, 7, InfixOp::Div),
                Token::Percent => (6, 7, InfixOp::Mod),
                Token::Power => (8, 7, InfixOp::Pow), // Right-associative
                _ => break,
            };

            if left_bp < min_bp {
                break;
            }

            self.next(); // consume operator

            lhs = match op {
                InfixOp::Add => {
                    let rhs = self.parse_expression(right_bp)?;
                    lhs + rhs
                }
                InfixOp::Sub => {
                    let rhs = self.parse_expression(right_bp)?;
                    lhs - rhs
                }
                InfixOp::Mul => {
                    let rhs = self.parse_expression(right_bp)?;
                    lhs * rhs
                }
                InfixOp::Div => {
                    let rhs = self.parse_expression(right_bp)?;
                    if rhs == 0.0 {
                        return Err(FormulaError::DivisionByZero);
                    }
                    lhs / rhs
                }
                InfixOp::Mod => {
                    let rhs = self.parse_expression(right_bp)?;
                    if rhs == 0.0 {
                        return Err(FormulaError::DivisionByZero);
                    }
                    lhs % rhs
                }
                InfixOp::Pow => {
                    let rhs = self.parse_expression(right_bp)?;
                    lhs.powf(rhs)
                }
                InfixOp::Eq => {
                    let rhs = self.parse_expression(right_bp)?;
                    if (lhs - rhs).abs() < 1e-9 {
                        1.0
                    } else {
                        0.0
                    }
                }
                InfixOp::Ne => {
                    let rhs = self.parse_expression(right_bp)?;
                    if (lhs - rhs).abs() >= 1e-9 {
                        1.0
                    } else {
                        0.0
                    }
                }
                InfixOp::Lt => {
                    let rhs = self.parse_expression(right_bp)?;
                    if lhs < rhs {
                        1.0
                    } else {
                        0.0
                    }
                }
                InfixOp::Le => {
                    let rhs = self.parse_expression(right_bp)?;
                    if lhs <= rhs {
                        1.0
                    } else {
                        0.0
                    }
                }
                InfixOp::Gt => {
                    let rhs = self.parse_expression(right_bp)?;
                    if lhs > rhs {
                        1.0
                    } else {
                        0.0
                    }
                }
                InfixOp::Ge => {
                    let rhs = self.parse_expression(right_bp)?;
                    if lhs >= rhs {
                        1.0
                    } else {
                        0.0
                    }
                }
                InfixOp::And => {
                    let rhs = self.parse_expression(right_bp)?;
                    if lhs != 0.0 && rhs != 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                InfixOp::Or => {
                    let rhs = self.parse_expression(right_bp)?;
                    if lhs != 0.0 || rhs != 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                InfixOp::Ternary => {
                    let true_val = self.parse_expression(0)?;
                    self.expect(Token::Colon)?;
                    let false_val = self.parse_expression(1)?;
                    if lhs != 0.0 {
                        true_val
                    } else {
                        false_val
                    }
                }
            };
        }

        Ok(lhs)
    }
}

fn eval_function(name: &str, args: &[f64]) -> Result<f64, FormulaError> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "round" => {
            if args.is_empty() || args.len() > 2 {
                return Err(FormulaError::InvalidArguments {
                    func: name.to_string(),
                    expected: 1,
                    got: args.len(),
                });
            }
            if args.len() == 1 {
                Ok(args[0].round())
            } else {
                let decimals = args[1] as i32;
                let factor = 10f64.powi(decimals);
                Ok((args[0] * factor).round() / factor)
            }
        }
        "trunc" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].trunc())
        }
        "abs" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].abs())
        }
        "min" => {
            if args.is_empty() {
                return Err(FormulaError::InvalidArguments {
                    func: name.to_string(),
                    expected: 2,
                    got: 0,
                });
            }
            Ok(args.iter().copied().fold(f64::INFINITY, f64::min))
        }
        "max" => {
            if args.is_empty() {
                return Err(FormulaError::InvalidArguments {
                    func: name.to_string(),
                    expected: 2,
                    got: 0,
                });
            }
            Ok(args.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        }
        "clamp" => {
            check_arg_count(name, args, 3)?;
            let val = args[0];
            let min = args[1];
            let max = args[2];
            Ok(val.max(min).min(max))
        }
        "sin" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].sin())
        }
        "cos" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].cos())
        }
        "tan" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].tan())
        }
        "asin" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].asin())
        }
        "acos" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].acos())
        }
        "atan" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].atan())
        }
        "atan2" => {
            check_arg_count(name, args, 2)?;
            Ok(args[0].atan2(args[1]))
        }
        "floor" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].floor())
        }
        "ceil" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].ceil())
        }
        "sqrt" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].sqrt())
        }
        "log" | "ln" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].ln())
        }
        "log10" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].log10())
        }
        "exp" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].exp())
        }
        "rad" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].to_radians())
        }
        "deg" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].to_degrees())
        }
        "sgn" | "sign" => {
            check_arg_count(name, args, 1)?;
            if args[0] > 0.0 {
                Ok(1.0)
            } else if args[0] < 0.0 {
                Ok(-1.0)
            } else {
                Ok(0.0)
            }
        }
        "frac" => {
            check_arg_count(name, args, 1)?;
            Ok(args[0].fract())
        }
        _ => Err(FormulaError::UnknownFunction(name.to_string())),
    }
}

fn check_arg_count(name: &str, args: &[f64], expected: usize) -> Result<(), FormulaError> {
    if args.len() != expected {
        Err(FormulaError::InvalidArguments {
            func: name.to_string(),
            expected,
            got: args.len(),
        })
    } else {
        Ok(())
    }
}

/// Evaluates a mathematical formula with variable substitution and standard functions.
pub fn eval_formula<V: VariableLookup>(expr: &str, vars: &V) -> Result<f64, FormulaError> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Err(FormulaError::EmptyExpression);
    }
    let tokens = tokenize(trimmed)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        vars,
    };
    let result = parser.parse_expression(0)?;
    if *parser.peek() != Token::Eof {
        return Err(FormulaError::SyntaxError(format!(
            "Unexpected trailing tokens in formula: {:?}",
            parser.peek()
        )));
    }
    Ok(result)
}
