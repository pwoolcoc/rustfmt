// rustfmt/main.rs

use std::io;
use std::str;
use syntax::parse::lexer::{StringReader, TokenAndSpan};
use syntax::parse::lexer;
use syntax::parse::token::Token;
use syntax::parse::token::keywords;
use syntax::parse::token;
use syntax::parse;

static TAB_WIDTH: i32 = 4;

enum ProductionToParse {
    MatchProduction,
    BracesProduction,
    ParenthesesProduction,
}

struct LineToken {
    token_and_span: TokenAndSpan,
    x_pos: i32,
}

impl LineToken {
    fn new(token_and_span: TokenAndSpan) -> LineToken {
        LineToken {
            token_and_span: token_and_span,
            x_pos: 0,
        }
    }

    fn whitespace_needed_after(&self, next: &LineToken) -> bool {
        match (&self.token_and_span.tok, &next.token_and_span.tok) {
            (&token::IDENT(..), &token::IDENT(..)) => true,
            (&token::IDENT(..), &token::NOT) => {
                // Macros.
                false
            }

            (&token::COLON, _) => true,
            (&token::COMMA, _) => true,
            (&token::EQ, _) | (_, &token::EQ) => true,
            (&token::LT, _) | (_, &token::LT) => true,
            (&token::LE, _) | (_, &token::LE) => true,
            (&token::EQEQ, _) | (_, &token::EQEQ) => true,
            (&token::NE, _) | (_, &token::NE) => true,
            (&token::GE, _) | (_, &token::GE) => true,
            (&token::GT, _) | (_, &token::GT) => true,
            (&token::ANDAND, _) | (_, &token::ANDAND) => true,
            (&token::OROR, _) | (_, &token::OROR) => true,
            (&token::TILDE, _) | (_, &token::TILDE) => true,

            (&token::LPAREN, _) => false,
            (_, &token::RPAREN) => false,
            (&token::BINOP(token::AND), _) => false,

            (&token::BINOP(_), _) | (_, &token::BINOP(_)) => true,
            (&token::BINOPEQ(_), _) | (_, &token::BINOPEQ(_)) => true,

            (&token::MOD_SEP, _) | (_, &token::MOD_SEP) => false,

            (&token::RARROW, _) | (_, &token::RARROW) => true,
            (&token::FAT_ARROW, _) | (_, &token::FAT_ARROW) => true,
            (&token::LBRACE, _) | (_, &token::LBRACE) => true,
            (&token::RBRACE, _) | (_, &token::RBRACE) => true,
            _ => false,
        }
    }

    fn length(&self) -> i32 {
        token::to_str(&self.token_and_span.tok).len() as i32
    }

    fn starts_logical_line(&self) -> bool {
        match self.token_and_span.tok {
            token::RBRACE => true,
            _ => false,
        }
    }

    fn preindentation(&self) -> i32 {
        match self.token_and_span.tok {
            token::RBRACE => -TAB_WIDTH,
            _ => 0,
        }
    }
}

struct LogicalLine {
    tokens: Vec<LineToken>,
}

impl LogicalLine {
    fn new() -> LogicalLine {
        LogicalLine {
            tokens: Vec::new(),
        }
    }

    fn layout(&mut self, mut x_pos: i32) {
        if self.tokens.len() == 0 {
            return
        }

        for i in range(0, self.tokens.len()) {
            self.tokens.get_mut(i).x_pos = x_pos;
            x_pos += self.tokens.get(i).length();

            if i < self.tokens.len() - 1 &&
                    self.tokens.get(i).whitespace_needed_after(self.tokens.get(i + 1)) {
                x_pos += 1;
            }
        }
    }

    fn whitespace_after(&self, index: uint) -> i32 {
        if self.tokens.len() <= 1 || index >= self.tokens.len() - 1 {
            return 0
        }

        self.tokens.get(index + 1).x_pos - (self.tokens.get(index).x_pos +
                                            self.tokens.get(index).length())
    }

    fn postindentation(&self) -> i32 {
        match self.tokens.as_slice().last() {
            None => 0,
            Some(line_token) => {
                match line_token.token_and_span.tok {
                    token::LBRACE => TAB_WIDTH,
                    _ => 0,
                }
            }
        }
    }
}

struct Formatter<'a> {
    lexer: StringReader<'a>,
    indent: i32,
    logical_line: LogicalLine,
    last_token: Token,
    newline_after_comma: bool,
}

impl<'a> Formatter<'a> {
    fn new<'a>(lexer: StringReader<'a>) -> Formatter<'a> {
        Formatter {
            lexer: lexer,
            indent: 0,
            logical_line: LogicalLine::new(),
            last_token: token::SEMI,
            newline_after_comma: false,
        }
    }

    fn token_ends_logical_line(&self, line_token: &LineToken) -> bool {
        match line_token.token_and_span.tok {
            token::LBRACE | token::SEMI | token::RBRACE => true,
            token::COMMA => self.newline_after_comma,
            _ => false,
        }
    }

    fn parse_tokens_up_to(&mut self, pred: |&token::Token| -> bool) -> bool {
        while self.next_token() {
            if pred(&self.last_token) {
                return true;
            }
        }
        return false;
    }

    fn parse_productions_up_to(&mut self, pred: |&token::Token| -> bool) -> bool {
        while self.next_token() {
            if pred(&self.last_token) {
                return true;
            }
            self.parse_production();
        }
        return false;
    }

    fn parse_match(&mut self) -> bool {
        // We've already parsed the keyword. Parse until we find a `{`.
        if !self.parse_tokens_up_to(|token| *token == token::LBRACE) {
            return false;
        }

        let old_newline_after_comma_setting = self.newline_after_comma;
        self.newline_after_comma = true;

        if !self.parse_productions_up_to(|token| *token == token::RBRACE) {
            return false;
        }

        self.newline_after_comma = old_newline_after_comma_setting;
        return true;
    }

    fn parse_braces(&mut self) -> bool {
        let old_newline_after_comma_setting = self.newline_after_comma;
        self.newline_after_comma = true;

        // We've already parsed the '{'. Parse until we find a '}'.
        let result = self.parse_productions_up_to(|token| *token == token::RBRACE);

        self.newline_after_comma = old_newline_after_comma_setting;
        return result;
    }

    fn parse_parentheses(&mut self) -> bool {
        let old_newline_after_comma_setting = self.newline_after_comma;
        self.newline_after_comma = false;

        // We've already parsed the '('. Parse until we find a ')'.
        let result = self.parse_productions_up_to(|token| *token == token::RPAREN);

        self.newline_after_comma = old_newline_after_comma_setting;
        return result;
    }

    fn parse_production(&mut self) -> bool {
        let production_to_parse;
        match self.last_token {
            token::IDENT(..) if token::is_keyword(keywords::Match, &self.last_token) => {
                production_to_parse = MatchProduction;
            }
            token::LBRACE => production_to_parse = BracesProduction,
            token::LPAREN => production_to_parse = ParenthesesProduction,
            _ => return true,
        }

        match production_to_parse {
            MatchProduction => return self.parse_match(),
            BracesProduction => return self.parse_braces(),
            ParenthesesProduction => return self.parse_parentheses(),
        }
    }

    fn next_token(&mut self) -> bool {
        use syntax::parse::lexer::Reader;

        loop {
            if self.lexer.is_eof() {
                return false;
            }

            let last_token = self.lexer.peek();
            self.last_token = last_token.tok.clone();
            let line_token = LineToken::new(last_token);
            if line_token.starts_logical_line() && self.logical_line.tokens.len() > 0 {
                self.flush_line();
                continue;
            }

            if self.logical_line.tokens.len() == 0 {
                self.indent += line_token.preindentation();
            }

            drop(self.lexer.next_token());
            let token_ends_logical_line = self.token_ends_logical_line(&line_token);
            self.logical_line.tokens.push(line_token);
            if token_ends_logical_line {
                self.flush_line();
            }

            return true;
        }
    }

    fn flush_line(&mut self) {
        self.logical_line.layout(self.indent);

        for _ in range(0, self.indent) {
            print!(" ");
        }
        for i in range(0, self.logical_line.tokens.len()) {
            print!("{}", token::to_str(&self.logical_line.tokens.get(i).token_and_span.tok));
            for _ in range(0, self.logical_line.whitespace_after(i)) {
                print!(" ");
            }
        }
        println!("");

        self.indent += self.logical_line.postindentation();
        self.logical_line = LogicalLine::new();
    }
}

#[main]
pub fn main() {
    let source = io::stdin().read_to_end().unwrap();
    let source = str::from_utf8(source.as_slice()).unwrap();

    let session = parse::new_parse_sess();
    let filemap = parse::string_to_filemap(&session, source.to_strbuf(), "<stdin>".to_strbuf());
    let lexer = lexer::new_string_reader(&session.span_diagnostic, filemap);
    let mut formatter = Formatter::new(lexer);

    while formatter.next_token() {
        formatter.parse_production();
    }
}
