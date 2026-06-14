use std::{iter::Peekable, str::CharIndices};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Word,
    String,
    Equals,
    Open,
    Close,
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) text: String,
    pub(crate) line: usize,
    pub(crate) start: usize,
}

pub(crate) fn tokenize(content: &str) -> Vec<Token> {
    Lexer::new(content).collect()
}

struct Lexer<'a> {
    chars: Peekable<CharIndices<'a>>,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn new(content: &'a str) -> Self {
        Self {
            chars: content.char_indices().peekable(),
            line: 1,
        }
    }

    fn next_char(&mut self) -> Option<(usize, char)> {
        let next = self.chars.next()?;
        if next.1 == '\n' {
            self.line += 1;
        }
        Some(next)
    }

    fn consume_comment(&mut self) {
        while let Some((_, next)) = self.next_char() {
            if next == '\n' {
                break;
            }
        }
    }

    fn consume_token(&mut self, start: usize, character: char) -> Option<Token> {
        match character {
            '\n' => None,
            '#' => {
                self.consume_comment();
                None
            }
            '"' => Some(self.consume_string(start)),
            '=' => Some(self.symbol_token(TokenKind::Equals, "=", start)),
            '{' => Some(self.symbol_token(TokenKind::Open, "{", start)),
            '}' => Some(self.symbol_token(TokenKind::Close, "}", start)),
            character if character.is_whitespace() => None,
            character => Some(self.consume_word(start, character)),
        }
    }

    fn symbol_token(&self, kind: TokenKind, text: &str, start: usize) -> Token {
        Token {
            kind,
            text: text.to_string(),
            line: self.line,
            start,
        }
    }

    fn consume_string(&mut self, start: usize) -> Token {
        let start_line = self.line;
        let mut text = String::new();
        let mut escaped = false;

        while let Some((_, next)) = self.next_char() {
            if escaped {
                text.push(next);
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == '"' {
                break;
            }
            text.push(next);
        }

        Token {
            kind: TokenKind::String,
            text,
            line: start_line,
            start,
        }
    }

    fn consume_word(&mut self, start: usize, first: char) -> Token {
        let start_line = self.line;
        let mut text = String::from(first);

        while let Some((_, next)) = self.chars.peek().copied() {
            if next.is_whitespace() || matches!(next, '=' | '{' | '}' | '#') {
                break;
            }
            self.next_char();
            text.push(next);
        }

        Token {
            kind: TokenKind::Word,
            text,
            line: start_line,
            start,
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((index, character)) = self.next_char() {
            if let Some(token) = self.consume_token(index, character) {
                return Some(token);
            }
        }

        None
    }
}
