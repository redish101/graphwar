use std::fmt;

#[derive(Debug)]
pub enum TokenType {
    // Literals
    Number(f64),

    // Identifiers
    Identifier(String),

    // Arithmetic operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,

    // Delimiters
    LeftParen,
    RightParen,
    Comma,

    // End of input
    EOF,
}

#[derive(Debug)]
pub struct Token {
    pub token_type: TokenType,

    pub start: usize,
    pub end: usize,
}

pub struct Lexer<'a> {
    input: &'a str,
    position: usize,
    current_char: Option<char>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    /// 遇到了无法识别的字符
    UnexpectedCharacter { ch: char, position: usize },
    /// 数字字面量无法解析为 f64（例如溢出）
    InvalidNumber {
        text: String,
        start: usize,
        end: usize,
    },
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexError::UnexpectedCharacter { ch, position } => {
                write!(f, "unexpected character '{}' at position {}", ch, position)
            }
            LexError::InvalidNumber { text, start, end } => {
                write!(f, "invalid number literal '{}' at {}..{}", text, start, end)
            }
        }
    }
}

impl std::error::Error for LexError {}
impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer {
            input,
            position: 0,
            current_char: None,
        };
        lexer.current_char = lexer.input.chars().next();
        lexer
    }

    pub fn skip_whitespace(&mut self) {
        while let Some(c) = self.current_char {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn advance(&mut self) {
        if let Some(c) = self.current_char {
            self.position += c.len_utf8();
            self.current_char = self.input[self.position..].chars().next();
        }
    }

    fn peek(&self) -> Option<char> {
        let c = self.current_char?;
        self.input[self.position + c.len_utf8()..].chars().next()
    }

    fn number(&mut self, start: usize) -> Result<TokenType, LexError> {
        let mut seen_dot = false;

        while let Some(c) = self.current_char {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' && !seen_dot && self.peek().map_or(false, |n| n.is_ascii_digit()) {
                seen_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        let end = self.position;
        let text = &self.input[start..end];

        text.parse::<f64>()
            .map(TokenType::Number)
            .map_err(|_| LexError::InvalidNumber {
                text: text.to_string(),
                start,
                end,
            })
    }

    fn identifier(&mut self, start: usize) -> TokenType {
        while let Some(c) = self.current_char {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        TokenType::Identifier(self.input[start..self.position].to_string())
    }

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace();

        let start = self.position;

        let token_type = match self.current_char {
            None => TokenType::EOF,

            Some(c) => match c {
                '+' => {
                    self.advance();
                    TokenType::Plus
                }
                '-' => {
                    self.advance();
                    TokenType::Minus
                }
                '*' => {
                    self.advance();
                    TokenType::Star
                }
                '/' => {
                    self.advance();
                    TokenType::Slash
                }
                '%' => {
                    self.advance();
                    TokenType::Percent
                }
                '^' => {
                    self.advance();
                    TokenType::Caret
                }

                '(' => {
                    self.advance();
                    TokenType::LeftParen
                }
                ')' => {
                    self.advance();
                    TokenType::RightParen
                }
                ',' => {
                    self.advance();
                    TokenType::Comma
                }

                c if c.is_ascii_digit()
                    || (c == '.' && self.peek().map_or(false, |n| n.is_ascii_digit())) =>
                {
                    self.number(start)?
                }

                c if c.is_alphabetic() || c == '_' => self.identifier(start),

                _ => {
                    return Err(LexError::UnexpectedCharacter {
                        ch: c,
                        position: start,
                    });
                }
            },
        };

        Ok(Token {
            token_type,
            start,
            end: self.position,
        })
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token()?;
            let is_eof = matches!(token.token_type, TokenType::EOF);
            tokens.push(token);

            if is_eof {
                break;
            }
        }

        Ok(tokens)
    }
}
