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
}

pub(crate) fn tokenize(content: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = content.chars().peekable();
    let mut line = 1usize;

    while let Some(character) = chars.next() {
        match character {
            '\n' => line += 1,
            '#' => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        line += 1;
                        break;
                    }
                }
            }
            '"' => {
                let start_line = line;
                let mut text = String::new();
                let mut escaped = false;
                for next in chars.by_ref() {
                    if next == '\n' {
                        line += 1;
                    }
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
                tokens.push(Token {
                    kind: TokenKind::String,
                    text,
                    line: start_line,
                });
            }
            '=' => tokens.push(Token {
                kind: TokenKind::Equals,
                text: "=".to_string(),
                line,
            }),
            '{' => tokens.push(Token {
                kind: TokenKind::Open,
                text: "{".to_string(),
                line,
            }),
            '}' => tokens.push(Token {
                kind: TokenKind::Close,
                text: "}".to_string(),
                line,
            }),
            character if character.is_whitespace() => {}
            character => {
                let start_line = line;
                let mut text = String::from(character);
                while let Some(next) = chars.peek().copied() {
                    if next.is_whitespace() || matches!(next, '=' | '{' | '}' | '#') {
                        break;
                    }
                    text.push(next);
                    chars.next();
                }
                tokens.push(Token {
                    kind: TokenKind::Word,
                    text,
                    line: start_line,
                });
            }
        }
    }

    tokens
}
