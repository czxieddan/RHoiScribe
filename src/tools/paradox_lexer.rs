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
    let mut tokens = Vec::new();
    let mut lexer = Lexer::new(content);

    while let Some((index, character)) = lexer.next_char() {
        match character {
            '\n' => {}
            '#' => lexer.consume_comment(),
            '"' => tokens.push(lexer.consume_string(index)),
            '=' => tokens.push(Token {
                kind: TokenKind::Equals,
                text: "=".to_string(),
                line: lexer.line,
                start: index,
            }),
            '{' => tokens.push(Token {
                kind: TokenKind::Open,
                text: "{".to_string(),
                line: lexer.line,
                start: index,
            }),
            '}' => tokens.push(Token {
                kind: TokenKind::Close,
                text: "}".to_string(),
                line: lexer.line,
                start: index,
            }),
            character if character.is_whitespace() => {}
            character => tokens.push(lexer.consume_word(index, character)),
        }
    }

    tokens
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
