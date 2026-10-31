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

    fn number(&mut self) -> TokenType {
        let start = self.position;
        let mut seen_dot = false;

        while let Some(c) = self.current_char {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.'
                && !seen_dot
                && self.peek().map_or(false, |n| n.is_ascii_digit())
            {
                seen_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        let text = &self.input[start..self.position];
        TokenType::Number(text.parse().expect("Invalid number literal"))
    }

    fn identifier(&mut self) -> TokenType {
        let start = self.position;

        while let Some(c) = self.current_char {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        TokenType::Identifier(self.input[start..self.position].to_string())
    }

    pub fn next_token(&mut self) -> Token {
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
                    || (c == '.'
                        && self.peek().map_or(false, |n| n.is_ascii_digit())) =>
                {
                    self.number()
                }

                c if c.is_alphabetic() || c == '_' => self.identifier(),

                _ => panic!("Unexpected character: '{}'", c),
            },
        };

        Token {
            token_type,
            start,
            end: self.position,
        }
    }

    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token();
            let is_eof = matches!(token.token_type, TokenType::EOF);
            tokens.push(token);

            if is_eof {
                break;
            }
        }

        tokens
    }
}
