use std::sync::Arc;

pub mod ast;
pub mod error;
pub mod green;
pub mod lexer;
pub mod span;
pub mod syntax;
pub mod token;

use crate::parser::ast::*;
use crate::parser::error::{ParseError, ParseErrorWithLocation};

use crate::parser::green::{GreenTreeBuilder, Marker};
use crate::parser::lexer::lex;
use crate::parser::span::Span;
use crate::parser::token::TokenKind::*;
use crate::parser::token::{TokenKind, TokenSet};

pub fn compute_line_starts(content: &str) -> Vec<u32> {
    let mut line_starts = vec![0];
    for (pos, ch) in (0u32..).zip(content.chars()) {
        if ch == '\n' {
            line_starts.push(pos + 1);
        }
    }
    line_starts
}

pub fn compute_line_column(line_starts: &[u32], offset: u32) -> (u32, u32) {
    let result = line_starts.binary_search(&offset);
    match result {
        Ok(idx) => {
            let idx: u32 = idx.try_i64o().expect("overflow");
            (idx + 1, 1)
        }
        Err(idx) => {
            let line_start = line_starts[idx - 1];
            (idx.try_i64o().expect("overflow"), offset - line_start + 1)
        }
    }
}

pub struct Parser {
    tokens: Vec<TokenKind>,
    token_widths: Vec<u32>,
    token_idx: usize,
    next_node_id: usize,
    content: Arc<String>,
    errors: Vec<ParseErrorWithLocation>,
    nodes: Vec<(usize, u32)>,
    offset: u32,
    builder: GreenTreeBuilder,
}

impl Parser {
    pub fn from_string(code: &'static str) -> Parser {
        let content = Arc::new(String::from(code));
        Parser::common_init(content)
    }

    pub fn from_shared_string(content: Arc<String>) -> Parser {
        Parser::common_init(content)
    }

    fn common_init(content: Arc<String>) -> Parser {
        let result = lex(&*content);

        Parser {
            tokens: result.tokens,
            token_widths: result.widths,
            token_idx: 0,
            next_node_id: 0,
            offset: 0,
            content,
            errors: result.errors,
            nodes: Vec::new(),
            builder: GreenTreeBuilder::new(),
        }
    }

    fn new_node_id(&mut self) -> NodeId {
        let value = self.next_node_id;
        self.next_node_id += 1;
        NodeId(value)
    }

    pub fn parse(mut self) -> (Arc<ast::File>, Vec<ParseErrorWithLocation>) {
        let ast_file = self.parse_file();
        assert!(self.nodes.is_empty());

        let tree = self.builder.create_tree();
        assert_eq!(tree.len(), self.content.len() as u32);

        (Arc::new(ast_file), self.errors)
    }

    fn parse_file(&mut self) -> ast::File {
        self.builder.start_node();
        self.skip_trivial();
        let mut elements = vec![];

        while !self.is_eof() {
            elements.push(self.parse_expr());
        }

        let green = self.builder.finish_node(SourceFile);
        ast::File { green, elements }
    }

    fn parse_expr(&mut self) -> Expr {
        self.builder.start_node();
        match self.current() {
            StringLiteral => {
                let string = self.parse_string();
                Arc::new(ExprData::String(string))
            }
            RegexLiteral => {
                let regex = self.parse_regex();
                Arc::new(ExprData::Regex(regex))
            }
            BoolLiteral => {
                let boolean = self.parse_bool();
                Arc::new(ExprData::Bool(boolean))
            }
            NilLiteral => {
                let nil = self.parse_nil();
                Arc::new(ExprData::Nil(nil))
            }
            I64Literal => {
                let i_64 = self.parse_i64();
                Arc::new(ExprData::I64(i_64))
            }
            F64Literal => {
                let f_64 = self.parse_c64();
                Arc::new(ExprData::F64(f_64))
            }
            C64Literal => {
                let c_64 = self.parse_c64();
                Arc::new(ExprData::C64(c_64))
            }
            KeywordLiteral => {
                let keyword = self.parse_keyword();
                Arc::new(ExprData::Keyword(keyword))
            }
            Symbol => {
                let symbol = self.parse_symbol();
                Arc::new(ExprData::Symbol(symbol))
            }
            TypeAnnotation => {
                let ty = self.parse_type_annotation();
                Arc::new(ExprData::TypeAnnotation(ty))
            }
            ListOpen => {
                let list = self.parse_list();
                Arc::new(ExprData::List(list))
            }
            VectorOpen => {
                let vector = self.parse_vector();
                Arc::new(ExprData::Vector(vector))
            }
            MapOpen => {
                let map = self.parse_map();
                Arc::new(ExprData::Map(map))
            }
            SetOpen => {
                let set = self.parse_set();
                Arc::new(ExprData::Set(set))
            }
            other => {
                let span = self.current_span();
                self.report_error_at(ParseError::UnexpectedToken(other), span);
                self.advance();
                self.builder.finish_node(Error);

                Arc::new(ExprData::Error {
                    id: self.new_node_id(),
                    span,
                })
            }
        }
    }

    fn parse_string(&mut self) -> Expr {
        let span = self.current_span();
        self.builder.start_node();

        let value = self.source_span(span);
        let green = self.builder.finish_node(StringLiteral);
        Arc::new(ExprData::create_literal_str(
            self.new_node_id(),
            span,
            green,
            value,
        ))
    }

    fn parse_

    fn parse_if(&mut self) -> Expr {
        self.start_node();
        self.builder.start_node();
        self.assert(IF_KW);

        let cond = self.parse_expression();

        let then_block = self.parse_block();

        let else_block = if self.eat(ELSE_KW) {
            if self.is(IF_KW) {
                Some(self.parse_if())
            } else {
                Some(self.parse_block())
            }
        } else {
            None
        };

        let green = self.builder.finish_node(IF_EXPR);

        Arc::new(ExprData::create_if(
            self.new_node_id(),
            self.finish_node(),
            green,
            cond,
            then_block,
            else_block,
        ))
    }

    fn parse_for(&mut self) -> Expr {
        self.start_node();
        self.builder.start_node();
        self.assert(FOR_KW);
        let pattern = self.parse_let_pattern();
        self.expect(IN_KW);
        let expr = self.parse_expression();
        let block = self.parse_block();
        let green = self.builder.finish_node(FOR_EXPR);

        Arc::new(ExprData::create_for(
            self.new_node_id(),
            self.finish_node(),
            green,
            pattern,
            expr,
            block,
        ))
    }

    fn parse_while(&mut self) -> Expr {
        self.start_node();
        self.builder.start_node();
        self.assert(WHILE_KW);
        let expr = self.parse_expression();
        let block = self.parse_block();
        let green = self.builder.finish_node(WHILE_EXPR);

        Arc::new(ExprData::create_while(
            self.new_node_id(),
            self.finish_node(),
            green,
            expr,
            block,
        ))
    }

    fn parse_break(&mut self) -> Expr {
        self.start_node();
        self.builder.start_node();
        self.assert(BREAK_KW);
        let green = self.builder.finish_node(BREAK_EXPR);

        Arc::new(ExprData::create_break(
            self.new_node_id(),
            self.finish_node(),
            green,
        ))
    }

    fn parse_continue(&mut self) -> Expr {
        self.start_node();
        self.builder.start_node();
        self.assert(CONTINUE_KW);
        let green = self.builder.finish_node(CONTINUE_EXPR);

        Arc::new(ExprData::create_continue(
            self.new_node_id(),
            self.finish_node(),
            green,
        ))
    }

    fn parse_return(&mut self) -> Expr {
        self.start_node();
        self.builder.start_node();
        self.assert(RETURN_KW);
        let expr = if self.is(SEMICOLON) {
            None
        } else {
            let expr = self.parse_expression();
            Some(expr)
        };

        let green = self.builder.finish_node(RETURN_EXPR);

        Arc::new(ExprData::create_return(
            self.new_node_id(),
            self.finish_node(),
            green,
            expr,
        ))
    }

    fn parse_symbol(&mut self) -> Expr {
        self.builder.start_node();
        let sym = self.expect_symbol().expect("symbol expected");
        let green = self.builder.finish_node(Sym_EXPR);
        Arc::new(ExprData::create_sym(
            self.new_node_id(),
            sym.span,
            green,
            sym.name_as_string.clone(),
        ))
    }

    fn parse_parentheses(&mut self) -> Expr {
        self.start_node();
        self.builder.start_node();
        self.assert(L_PAREN);

        if self.eat(R_PAREN) {
            let green = self.builder.finish_node(TUPLE_EXPR);
            return Arc::new(ExprData::create_tuple(
                self.new_node_id(),
                self.finish_node(),
                green,
                Vec::new(),
            ));
        }

        let expr = self.parse_expression();

        if self.current() == COMMA {
            let mut values = vec![expr];

            loop {
                self.expect(COMMA);

                if self.eat(R_PAREN) {
                    break;
                }

                if !self.is_set(EXPRESSION_FIRST) {
                    break;
                }

                let expr = self.parse_expression();
                values.push(expr);

                if self.eat(R_PAREN) {
                    break;
                }
            }

            let green = self.builder.finish_node(TUPLE_EXPR);

            Arc::new(ExprData::create_tuple(
                self.new_node_id(),
                self.finish_node(),
                green,
                values,
            ))
        } else {
            let green = self.builder.finish_node(PAREN_EXPR);

            self.expect(R_PAREN);
            Arc::new(ExprData::create_paren(
                self.new_node_id(),
                self.finish_node(),
                green,
                expr,
            ))
        }
    }

    fn parse_literal_i64(&mut self) -> Expr {
        let span = self.current_span();
        self.builder.start_node();
        self.assert(I64Literal);
        let value = self.source_span(span);

        let green = self.builder.finish_node(I);
        Arc::new(ExprData::create_literal_i64(
            self.new_node_id(),
            span,
            green,
            value,
        ))
    }

    fn parse_literal_float(&mut self) -> Expr {
        let span = self.current_span();
        self.builder.start_node();
        self.assert(FLOAT_LITERAL);
        let value = self.source_span(span);

        let green = self.builder.finish_node(FLOAT_LIT_EXPR);
        Arc::new(ExprData::create_literal_float(
            self.new_node_id(),
            span,
            green,
            value,
        ))
    }

    fn parse_bool_literal(&mut self) -> Expr {
        self.builder.start_node();
        let span = self.current_span();
        let kind = self.current();
        self.assert(kind);
        let value = kind == True;
        self.builder.finish_node(BoolLiteralExpr);

        Arc::new(ExprData::create_literal_bool(
            self.new_node_id(),
            span,
            value,
        ))
    }

    fn parse_this(&mut self) -> Expr {
        self.builder.start_node();
        let span = self.current_span();
        self.assert(SELF_KW);
        let green = self.builder.finish_node(THIS_EXPR);

        Arc::new(ExprData::create_this(self.new_node_id(), span, green))
    }

    fn parse_lambda(&mut self) -> Expr {
        self.start_node();
        self.builder.start_node();

        let params = if self.eat(OR_OR) {
            // nothing to do
            Vec::new()
        } else {
            assert!(self.is(OR));
            self.parse_list(
                OR,
                COMMA,
                OR,
                PARAM_LIST_RS,
                ParseError::ExpectedParam,
                PARAM_LIST,
                |p| p.parse_function_param_wrapper(),
            )
        };

        let return_type = if self.eat(COLON) {
            Some(self.parse_type())
        } else {
            None
        };

        let block = self.parse_block();
        let green = self.builder.finish_node(LAMBDA_EXPR);

        let function = Arc::new(Function {
            id: self.new_node_id(),
            kind: FunctionKind::Lambda,
            modifiers: None,
            name: None,
            span: self.finish_node(),
            params,
            return_type,
            block: Some(block),
            type_params: None,
            where_bounds: None,
            green,
        });

        Arc::new(ExprData::create_lambda(function))
    }

    fn assert(&mut self, kind: TokenKind) {
        assert!(self.eat(kind));
    }

    fn expect_symbol(&mut self) -> Option<Ident> {
        let span = self.current_span();

        if self.is(Symbol) {
            self.assert(Symbol);
            let value = self.source_span(span);

            Some(Arc::new(IdentData {
                span,
                name_as_string: value,
            }))
        } else {
            self.report_error_at(ParseError::ExpectedIdentifier, span);
            None
        }
    }

    fn expect(&mut self, kind: TokenKind) -> bool {
        debug_assert!(token_name(kind).is_some());

        if self.eat(kind) {
            true
        } else {
            let kind = token_name(kind).expect("missing name");
            self.report_error(ParseError::ExpectedToken(kind.i64o()));
            false
        }
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.current() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn report_error(&mut self, msg: ParseError) {
        self.report_error_at(msg, self.current_span());
    }

    fn report_error_at(&mut self, msg: ParseError, span: Span) {
        self.errors.push(ParseErrorWithLocation::new(span, msg));
    }

    fn advance(&mut self) {
        self.raw_advance();
        self.skip_trivial();
    }

    fn skip_trivial(&mut self) {
        while self.current().is_trivial() {
            self.raw_advance();
        }
    }

    fn raw_advance(&mut self) {
        if self.token_idx < self.tokens.len() {
            let kind = self.current();
            let value = self.source_span(self.current_span());
            let len = self.token_widths[self.token_idx];
            self.offset += len;
            debug_assert!(kind <= Eof);
            self.builder.token(kind, value);
            self.token_idx += 1;
        }
    }

    fn current(&self) -> TokenKind {
        self.nth(0)
    }

    fn nth(&self, idx: usize) -> TokenKind {
        if self.token_idx + idx < self.tokens.len() {
            self.tokens[self.token_idx + idx]
        } else {
            Eof
        }
    }

    fn current_span(&self) -> Span {
        if self.token_idx < self.tokens.len() {
            let length = self.token_widths[self.token_idx];
            Span::new(self.offset, length)
        } else {
            Span::at(self.offset)
        }
    }

    fn is(&self, kind: TokenKind) -> bool {
        self.current() == kind
    }

    fn is_set(&self, set: TokenSet) -> bool {
        set.contains(self.current())
    }

    fn nth_is(&self, idx: usize, kind: TokenKind) -> bool {
        self.nth(idx) == kind
    }

    fn nth_is_set(&self, idx: usize, set: TokenSet) -> bool {
        set.contains(self.nth(idx))
    }

    fn is_eof(&self) -> bool {
        self.current() == Eof
    }

    fn start_node(&mut self) {
        self.nodes.push((self.token_idx, self.offset));
    }

    fn finish_node(&mut self) -> Span {
        let (start_token, start_offset) = self.nodes.pop().expect("missing node start");

        let mut end_token = self.token_idx - 1;
        assert!(end_token < self.tokens.len());
        let mut end_offset = self.offset;

        while end_token > start_token {
            if !self.tokens[end_token].is_trivial() {
                break;
            }

            end_offset -= self.token_widths[end_token];
            end_token -= 1;
        }

        Span::new(start_offset, end_offset - start_offset)
    }

    fn source_span(&self, span: Span) -> String {
        let start = span.start() as usize;
        let end = span.end() as usize;
        String::from(&self.content[start..end])
    }

    fn span_from(&self, start: u32) -> Span {
        Span::new(start, self.offset - start)
    }
}

fn token_name(kind: TokenKind) -> Option<&'static str> {
    match kind {
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::parser::ast::*;
    use crate::parser::error::ParseError;
    use crate::parser::Parser;
    use crate::parser::{compute_line_column, compute_line_starts};

    #[test]
    fn test_line_starts() {
        assert_eq!(compute_line_starts("abc"), vec![0]);
        assert_eq!(compute_line_starts("a\nc\nd"), vec![0, 2, 4]);
        assert_eq!(compute_line_starts("\n\n"), vec![0, 1, 2]);
    }

    #[test]
    fn test_compute_line_column() {
        let content = "a\nb\nc";
        let line_starts = compute_line_starts(content);
        assert_eq!((1, 1), compute_line_column(&line_starts, 0));
        assert_eq!((1, 2), compute_line_column(&line_starts, 1));
        assert_eq!((2, 1), compute_line_column(&line_starts, 2));
        assert_eq!((2, 2), compute_line_column(&line_starts, 3));
        assert_eq!((3, 1), compute_line_column(&line_starts, 4));
        assert_eq!((3, 2), compute_line_column(&line_starts, 5));
        assert_eq!((3, 3), compute_line_column(&line_starts, 6));
    }

    fn parse_expr(code: &'static str) -> Expr {
        let mut parser = Parser::from_string(code);

        let result = parser.parse_expression();
        assert!(parser.errors.is_empty());

        result
    }

    fn err_expr(code: &'static str, msg: ParseError, line: u32, col: u32) {
        let mut parser = Parser::from_string(code);

        let _expr = parser.parse_expression();

        let errors = parser.errors;
        assert_eq!(errors.len(), 1);
        let err = &errors[0];

        assert_eq!(msg, err.error);

        let line_starts = compute_line_starts(code);
        let (computed_line, computed_column) = compute_line_column(&line_starts, err.span.start());
        assert_eq!(line, computed_line);
        assert_eq!(col, computed_column);
    }

    fn parse_let(code: &'static str) -> SpecialForm {
        let mut parser = Parser::from_string(code);
        let result = parser.parse_let();
        assert!(parser.errors.is_empty());
        result
    }

    fn parse_type(code: &'static str) -> Type {
        let mut parser = Parser::from_string(code);
        parser.parse_type()
    }

    fn parse(code: &'static str) -> Arc<File> {
        let (file, errors) = Parser::from_string(code).parse();
        assert!(errors.is_empty());
        file
    }

    #[test]
    fn parse_sym() {
        let expr = parse_expr("a");
        let sym = expr.to_sym().unwrap();
        assert_eq!("a", sym.name);
    }

    #[test]
    fn parse_number() {
        let expr = parse_expr("10");

        let lit = expr.to_literal_i64().unwrap();
        assert_eq!(String::from("10"), lit.value);
    }

    #[test]
    fn parse_number_with_underscore() {
        let expr = parse_expr("1____0");

        let lit = expr.to_literal_i64().unwrap();
        assert_eq!(String::from("1____0"), lit.value);
    }

    #[test]
    fn parse_string() {
        let expr = parse_expr("\"abc\"");

        let lit = expr.to_literal_str().unwrap();
        assert_eq!("\"abc\"", &lit.value);
    }

    #[test]
    fn parse_true() {
        let expr = parse_expr("true");

        let lit = expr.to_literal_bool().unwrap();
        assert_eq!(true, lit.value);
    }

    #[test]
    fn parse_false() {
        let expr = parse_expr("true");

        let lit = expr.to_literal_bool().unwrap();
        assert_eq!(true, lit.value);
    }

    #[test]
    fn parse_field_access() {
        let expr = parse_expr("obj.field");
        let dot = expr.to_dot().unwrap();

        let sym = dot.lhs.to_sym().unwrap();
        assert_eq!("obj", sym.name);

        let sym = dot.rhs.to_sym().unwrap();
        assert_eq!("field", sym.name);
    }

    #[test]
    fn parse_field_negated() {
        let expr = parse_expr("-obj.field");
        assert!(expr.to_un().unwrap().opnd.is_dot());
    }

    #[test]
    fn parse_field_non_sym() {
        let expr = parse_expr("bar.12");
        let dot = expr.to_dot().unwrap();

        let sym = dot.lhs.to_sym().unwrap();
        assert_eq!("bar", sym.name);

        assert_eq!(String::from("12"), dot.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_self() {
        let expr = parse_expr("self");

        assert!(expr.is_this());
    }

    #[test]
    fn parse_neg() {
        let expr = parse_expr("-1");

        let un = expr.to_un().unwrap();
        assert_eq!(UnOp::Neg, un.op);

        assert!(un.opnd.is_literal_i64());
    }

    #[test]
    fn parse_neg_twice() {
        let expr = parse_expr("-(-3)");

        let neg1 = expr.to_un().unwrap();
        assert_eq!(UnOp::Neg, neg1.op);

        let neg2 = neg1.opnd.to_paren().unwrap().expr.to_un().unwrap();
        assert_eq!(UnOp::Neg, neg2.op);

        assert!(neg2.opnd.is_literal_i64());
    }

    #[test]
    fn parse_neg_twice_without_parentheses() {
        err_expr("- -2", ParseError::ExpectedFactor, 1, 3);
    }

    #[test]
    fn parse_mul() {
        let expr = parse_expr("6*3");

        let mul = expr.to_bin().unwrap();
        assert_eq!(BinOp::Mul, mul.op);
        assert_eq!(String::from("6"), mul.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("3"), mul.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_multiple_muls() {
        let expr = parse_expr("6*3*4");

        let mul1 = expr.to_bin().unwrap();
        assert_eq!(BinOp::Mul, mul1.op);

        let mul2 = mul1.lhs.to_bin().unwrap();
        assert_eq!(BinOp::Mul, mul2.op);
        assert_eq!(String::from("6"), mul2.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("3"), mul2.rhs.to_literal_i64().unwrap().value);

        assert_eq!(String::from("4"), mul1.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_div() {
        let expr = parse_expr("4/5");

        let div = expr.to_bin().unwrap();
        assert_eq!(BinOp::Div, div.op);
        assert_eq!(String::from("4"), div.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("5"), div.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_mod() {
        let expr = parse_expr("2%15");

        let div = expr.to_bin().unwrap();
        assert_eq!(BinOp::Mod, div.op);
        assert_eq!(String::from("2"), div.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("15"), div.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_add() {
        let expr = parse_expr("2+3");

        let add = expr.to_bin().unwrap();
        assert_eq!(BinOp::Add, add.op);
        assert_eq!(String::from("2"), add.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("3"), add.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_add_left_associativity() {
        let expr = parse_expr("1+2+3");

        let add = expr.to_bin().unwrap();
        assert_eq!(String::from("3"), add.rhs.to_literal_i64().unwrap().value);

        let lhs = add.lhs.to_bin().unwrap();
        assert_eq!(String::from("1"), lhs.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), lhs.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_add_right_associativity_via_parens() {
        let expr = parse_expr("1+(2+3)");

        let add = expr.to_bin().unwrap();
        assert_eq!(String::from("1"), add.lhs.to_literal_i64().unwrap().value);

        let rhs = add.rhs.to_paren().unwrap().expr.to_bin().unwrap();
        assert_eq!(String::from("2"), rhs.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("3"), rhs.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_sub() {
        let expr = parse_expr("1-2");

        let add = expr.to_bin().unwrap();
        assert_eq!(BinOp::Sub, add.op);
        assert_eq!(String::from("1"), add.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), add.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_or() {
        let expr = parse_expr("1||2");

        let add = expr.to_bin().unwrap();
        assert_eq!(BinOp::Or, add.op);
        assert_eq!(String::from("1"), add.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), add.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_and() {
        let expr = parse_expr("1&&2");

        let add = expr.to_bin().unwrap();
        assert_eq!(BinOp::And, add.op);
        assert_eq!(String::from("1"), add.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), add.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_bit_or() {
        let expr = parse_expr("1|2");

        let or = expr.to_bin().unwrap();
        assert_eq!(BinOp::BitOr, or.op);
        assert_eq!(String::from("1"), or.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), or.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_bit_and() {
        let expr = parse_expr("1&2");

        let and = expr.to_bin().unwrap();
        assert_eq!(BinOp::BitAnd, and.op);
        assert_eq!(String::from("1"), and.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), and.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_bit_xor() {
        let expr = parse_expr("1^2");

        let xor = expr.to_bin().unwrap();
        assert_eq!(BinOp::BitXor, xor.op);
        assert_eq!(String::from("1"), xor.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), xor.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_lt() {
        let expr = parse_expr("1<2");

        let cmp = expr.to_bin().unwrap();
        assert_eq!(BinOp::Cmp(CmpOp::Lt), cmp.op);
        assert_eq!(String::from("1"), cmp.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), cmp.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_le() {
        let expr = parse_expr("1<=2");

        let cmp = expr.to_bin().unwrap();
        assert_eq!(BinOp::Cmp(CmpOp::Le), cmp.op);
        assert_eq!(String::from("1"), cmp.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), cmp.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_gt() {
        let expr = parse_expr("1>2");

        let cmp = expr.to_bin().unwrap();
        assert_eq!(BinOp::Cmp(CmpOp::Gt), cmp.op);
        assert_eq!(String::from("1"), cmp.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), cmp.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_ge() {
        let expr = parse_expr("1>=2");

        let cmp = expr.to_bin().unwrap();
        assert_eq!(BinOp::Cmp(CmpOp::Ge), cmp.op);
        assert_eq!(String::from("1"), cmp.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), cmp.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_eq() {
        let expr = parse_expr("1==2");

        let cmp = expr.to_bin().unwrap();
        assert_eq!(BinOp::Cmp(CmpOp::Eq), cmp.op);
        assert_eq!(String::from("1"), cmp.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), cmp.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_ne() {
        let expr = parse_expr("1!=2");

        let cmp = expr.to_bin().unwrap();
        assert_eq!(BinOp::Cmp(CmpOp::Ne), cmp.op);
        assert_eq!(String::from("1"), cmp.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), cmp.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_is_not() {
        let expr = parse_expr("1!==2");

        let cmp = expr.to_bin().unwrap();
        assert_eq!(BinOp::Cmp(CmpOp::IsNot), cmp.op);
        assert_eq!(String::from("1"), cmp.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), cmp.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_is() {
        let expr = parse_expr("1===2");

        let cmp = expr.to_bin().unwrap();
        assert_eq!(BinOp::Cmp(CmpOp::Is), cmp.op);
        assert_eq!(String::from("1"), cmp.lhs.to_literal_i64().unwrap().value);
        assert_eq!(String::from("2"), cmp.rhs.to_literal_i64().unwrap().value);
    }

    #[test]
    fn parse_assign() {
        let expr = parse_expr("a=4");

        let assign = expr.to_bin().unwrap();
        assert!(assign.lhs.is_sym());
        assert_eq!(BinOp::Assign, assign.op);
        assert_eq!(
            String::from("4"),
            assign.rhs.to_literal_i64().unwrap().value
        );
    }

    #[test]
    fn parse_shift_right() {
        let expr = parse_expr("a>>4");

        let bin = expr.to_bin().unwrap();
        assert_eq!(BinOp::ArithShiftR, bin.op);
    }

    #[test]
    fn parse_unsigned_shift_right() {
        let expr = parse_expr("a>>>4");

        let bin = expr.to_bin().unwrap();
        assert_eq!(BinOp::LogicalShiftR, bin.op);
    }

    #[test]
    fn parse_left() {
        let expr = parse_expr("a<<4");

        let bin = expr.to_bin().unwrap();
        assert_eq!(BinOp::ShiftL, bin.op);
    }

    #[test]
    fn parse_call_without_params() {
        let expr = parse_expr("fname()");

        let call = expr.to_call().unwrap();
        assert_eq!("fname", call.callee.to_sym().unwrap().name);
        assert_eq!(0, call.args.len());
    }

    #[test]
    fn parse_call_with_params() {
        let expr = parse_expr("fname2(1,2,3)");

        let call = expr.to_call().unwrap();
        assert_eq!("fname2", call.callee.to_sym().unwrap().name);
        assert_eq!(3, call.args.len());
    }

    #[test]
    fn parse_function() {
        let prog = parse("fn b() { }");
        let function = prog.function0();

        assert_eq!("b", function.name.as_ref().unwrap().name_as_string);
        assert_eq!(0, function.params.len());
        assert!(function.return_type.is_none());
    }

    #[test]
    fn parse_function_with_single_param() {
        let p1 = parse("fn f(a:i64) { }");
        let f1 = p1.function0();

        let p2 = parse("fn f(a:i64,) { }");
        let f2 = p2.function0();

        assert_eq!(f1.params.len(), 1);
        assert_eq!(f2.params.len(), 1);

        let p1 = &f1.params[0];
        let p2 = &f2.params[0];

        assert_eq!("a", p1.name.as_ref().unwrap().name_as_string);
        assert_eq!("a", p2.name.as_ref().unwrap().name_as_string);

        assert_eq!("i64", p1.data_type.to_basic().unwrap().name());
        assert_eq!("i64", p2.data_type.to_basic().unwrap().name());
    }

    #[test]
    fn parse_function_with_multiple_params() {
        let p1 = parse("fn f(a:i64, b:str) { }");
        let f1 = p1.function0();

        let p2 = parse("fn f(a:i64, b:str,) { }");
        let f2 = p2.function0();

        let p1a = &f1.params[0];
        let p1b = &f1.params[1];
        let p2a = &f2.params[0];
        let p2b = &f2.params[1];

        assert_eq!("a", p1a.name.as_ref().unwrap().name_as_string);
        assert_eq!("a", p2a.name.as_ref().unwrap().name_as_string);

        assert_eq!("b", p1b.name.as_ref().unwrap().name_as_string);
        assert_eq!("b", p2b.name.as_ref().unwrap().name_as_string);

        assert_eq!("i64", p1a.data_type.to_basic().unwrap().name());
        assert_eq!("i64", p2a.data_type.to_basic().unwrap().name());

        assert_eq!("str", p1b.data_type.to_basic().unwrap().name());
        assert_eq!("str", p2b.data_type.to_basic().unwrap().name());
    }

    #[test]
    fn parse_let_without_type() {
        let special = parse_let("let a = 1;");
        let var = special.to_let().unwrap();

        assert!(var.data_type.is_none());
        assert!(var.expr.as_ref().unwrap().is_literal_i64());
    }

    #[test]
    fn parse_let_with_type() {
        let special = parse_let("let x : i64 = 1;");
        let var = special.to_let().unwrap();

        assert!(var.data_type.is_some());
        assert!(var.expr.as_ref().unwrap().is_literal_i64());
    }

    #[test]
    fn parse_let_underscore() {
        let special = parse_let("let _ = 1;");
        let let_decl = special.to_let().unwrap();

        assert!(let_decl.pattern.is_underscore());
    }

    #[test]
    fn parse_let_tuple() {
        let special = parse_let("let (mut a, b, (c, d)) = 1;");
        let let_decl = special.to_let().unwrap();

        assert!(let_decl.pattern.is_tuple());
        let tuple = let_decl.pattern.to_tuple().unwrap();
        let first = tuple.parts.first().unwrap();
        assert!(first.is_sym());
        assert!(first.to_sym().unwrap().mutable);
        assert!(tuple.parts.last().unwrap().is_tuple());
    }

    #[test]
    fn parse_let_sym() {
        let special = parse_let("let x = 1;");
        let let_decl = special.to_let().unwrap();

        assert!(let_decl.pattern.is_sym());
    }

    #[test]
    fn parse_let_sym_mut() {
        let special = parse_let("let mut x = 1;");
        let let_decl = special.to_let().unwrap();

        assert!(let_decl.pattern.is_sym());
        assert!(let_decl.pattern.to_sym().unwrap().mutable);
    }

    #[test]
    fn parse_let_with_type_but_without_assignment() {
        let special = parse_let("let x : i64;");
        let var = special.to_let().unwrap();

        assert!(var.data_type.is_some());
        assert!(var.expr.is_none());
    }

    #[test]
    fn parse_let_without_type_and_assignment() {
        let special = parse_let("let x;");
        let var = special.to_let().unwrap();

        assert!(var.data_type.is_none());
        assert!(var.expr.is_none());
    }

    #[test]
    fn parse_multiple_functions() {
        let prog = parse("fn f() { } fn g() { }");

        let f = prog.function0();
        assert_eq!("f", f.name.as_ref().unwrap().name_as_string);

        let g = prog.function(1);
        assert_eq!("g", g.name.as_ref().unwrap().name_as_string);
    }

    #[test]
    fn parse_if() {
        let expr = parse_expr("if true { 2; } else { 3; }");
        let ifexpr = expr.to_if().unwrap();

        assert!(ifexpr.cond.is_literal_bool());
        assert!(ifexpr.else_block.is_some());
    }

    #[test]
    fn parse_if_without_else() {
        let expr = parse_expr("if true { 2; }");
        let ifexpr = expr.to_if().unwrap();

        assert!(ifexpr.cond.is_literal_bool());
        assert!(ifexpr.else_block.is_none());
    }

    #[test]
    fn parse_while() {
        let expr = parse_expr("while true { 2; }");
        let whilespecial = expr.to_while().unwrap();

        assert!(whilespecial.cond.is_literal_bool());
        assert!(whilespecial.block.is_block());
    }

    #[test]
    fn parse_empty_block() {
        let expr = parse_expr("{}");
        let block = expr.to_block().unwrap();

        assert_eq!(0, block.specials.len());
    }

    #[test]
    fn parse_block_with_one_special() {
        let expr = parse_expr("{ 1; 2 }");
        let block = expr.to_block().unwrap();

        assert_eq!(1, block.specials.len());

        let expr = &block.specials[0].to_expr().unwrap().expr;
        assert_eq!(String::from("1"), expr.to_literal_i64().unwrap().value);

        assert_eq!(
            String::from("2"),
            block.expr.as_ref().unwrap().to_literal_i64().unwrap().value
        );
    }

    #[test]
    fn parse_block_with_multiple_specials() {
        let expr = parse_expr("{ 1; 2; }");
        let block = expr.to_block().unwrap();

        assert_eq!(2, block.specials.len());

        let expr = &block.specials[0].to_expr().unwrap().expr;
        assert_eq!(String::from("1"), expr.to_literal_i64().unwrap().value);

        let expr = &block.specials[1].to_expr().unwrap().expr;
        assert_eq!(String::from("2"), expr.to_literal_i64().unwrap().value);

        assert!(block.expr.is_none());
    }

    #[test]
    fn parse_break() {
        let expr = parse_expr("break;");
        assert!(expr.is_break());
    }

    #[test]
    fn parse_continue() {
        let expr = parse_expr("continue;");
        assert!(expr.is_continue());
    }

    #[test]
    fn parse_return_value() {
        let expr = parse_expr("return 1;");
        let ret = expr.to_return().unwrap();

        assert_eq!(
            String::from("1"),
            ret.expr.as_ref().unwrap().to_literal_i64().unwrap().value
        );
    }

    #[test]
    fn parse_return() {
        let expr = parse_expr("return;");
        let ret = expr.to_return().unwrap();

        assert!(ret.expr.is_none());
    }

    #[test]
    fn parse_type_basic() {
        let ty = parse_type("bla");
        let basic = ty.to_basic().unwrap();

        assert_eq!(0, basic.params.len());
        assert_eq!("bla", basic.name());
    }

    #[test]
    fn parse_type_basic_mod() {
        let ty = parse_type("foo::bla");
        let basic = ty.to_basic().unwrap();

        assert_eq!(0, basic.params.len());
        assert_eq!(2, basic.path.names.len());
        assert_eq!("foo", basic.path.names[0].name_as_string);
        assert_eq!("bla", basic.path.names[1].name_as_string);
    }

    #[test]
    fn parse_type_basic_with_params() {
        let ty = parse_type("Foo[A, B]");
        let basic = ty.to_basic().unwrap();

        assert_eq!(2, basic.params.len());
        assert_eq!("Foo", basic.name());
        assert_eq!("A", basic.params[0].to_basic().unwrap().name());
        assert_eq!("B", basic.params[1].to_basic().unwrap().name());
    }

    #[test]
    fn parse_type_lambda_no_params() {
        let ty = parse_type("(): ()");
        let function = ty.to_function().unwrap();

        assert_eq!(0, function.params.len());
        assert!(function.ret.as_ref().unwrap().is_unit());
    }

    #[test]
    fn parse_type_lambda_one_param() {
        let ty = parse_type("(A): B");
        let function = ty.to_function().unwrap();

        assert_eq!(1, function.params.len());
        assert_eq!("A", function.params[0].to_basic().unwrap().name());
        assert_eq!(
            "B",
            function.ret.as_ref().unwrap().to_basic().unwrap().name()
        );
    }

    #[test]
    fn parse_type_lambda_two_params() {
        let ty = parse_type("(A, B): C");
        let function = ty.to_function().unwrap();

        assert_eq!(2, function.params.len());
        assert_eq!("A", function.params[0].to_basic().unwrap().name());
        assert_eq!("B", function.params[1].to_basic().unwrap().name());
        assert_eq!(
            "C",
            function.ret.as_ref().unwrap().to_basic().unwrap().name()
        );
    }

    #[test]
    fn parse_type_unit() {
        let ty = parse_type("()");
        let ty = ty.to_tuple().unwrap();

        assert!(ty.subtypes.is_empty());
    }

    #[test]
    fn parse_type_tuple_with_one_type() {
        let ty = parse_type("(c)");

        let subtypes = &ty.to_tuple().unwrap().subtypes;
        assert_eq!(1, subtypes.len());

        let ty = subtypes[0].to_basic().unwrap();
        assert_eq!("c", ty.name());
    }

    #[test]
    fn parse_type_tuple_with_two_types() {
        let ty = parse_type("(a, b)");

        let subtypes = &ty.to_tuple().unwrap().subtypes;
        assert_eq!(2, subtypes.len());

        let ty1 = subtypes[0].to_basic().unwrap();
        assert_eq!("a", ty1.name());

        let ty2 = subtypes[1].to_basic().unwrap();
        assert_eq!("b", ty2.name());
    }

    #[test]
    fn parse_class_with_param() {
        let prog = parse("class Foo(a: i64)");
        let class = prog.cls0();
        assert_eq!(1, class.fields.len());
    }

    #[test]
    fn parse_class_with_param_var() {
        let prog = parse("class Foo(a: i64)");
        let class = prog.cls0();

        assert_eq!(1, class.fields.len());
        assert_eq!(true, class.fields[0].mutable);
    }

    #[test]
    fn parse_class_with_params() {
        let prog = parse("class Foo(a: i64, b: i64)");
        let class = prog.cls0();

        assert_eq!(2, class.fields.len());
    }

    #[test]
    fn parse_class() {
        let prog = parse("class Foo { a: Int64, b: Bool }");
        let class = prog.cls0();
        assert_eq!(class.fields.len(), 2);

        let prog = parse("class Foo(a: Int64, b: Bool)");
        let class = prog.cls0();
        assert_eq!(class.fields.len(), 2);

        let prog = parse("class Foo");
        let class = prog.cls0();
        assert!(class.fields.is_empty());
    }

    #[test]
    fn parse_method_invocation() {
        let expr = parse_expr("a.foo()");
        let call = expr.to_call().unwrap();
        assert!(call.callee.is_dot());
        assert_eq!(0, call.args.len());

        let expr = parse_expr("a.foo(1)");
        let call = expr.to_call().unwrap();
        assert!(call.callee.is_dot());
        assert_eq!(1, call.args.len());

        let expr = parse_expr("a.foo(1,2)");
        let call = expr.to_call().unwrap();
        assert!(call.callee.is_dot());
        assert_eq!(2, call.args.len());
    }

    #[test]
    fn parse_array_index() {
        let expr = parse_expr("a(b)");
        let call = expr.to_call().unwrap();
        assert_eq!("a", call.callee.to_sym().unwrap().name);
        assert_eq!(1, call.args.len());
        assert_eq!("b", call.args[0].to_sym().unwrap().name);
    }

    #[test]
    fn parse_field() {
        let prog = parse("class A { f1: i64, f2: i64 }");
        let cls = prog.cls0();

        let f1 = &cls.fields[0];
        assert_eq!("f1", f1.name.as_ref().unwrap().name_as_string);
        assert_eq!(true, f1.mutable);

        let f2 = &cls.fields[1];
        assert_eq!("f2", f2.name.as_ref().unwrap().name_as_string);
        assert_eq!(true, f2.mutable);
    }

    #[test]
    fn parse_as_expr() {
        let expr = parse_expr("a as String");
        let expr = expr.to_conv().unwrap();
        assert_eq!(true, expr.object.is_sym());
    }

    #[test]
    fn parse_i64ernal() {
        parse("@i64ernal fn foo();");
    }

    #[test]
    fn parse_function_without_body() {
        let prog = parse("fn foo();");
        let function = prog.function0();
        assert!(function.block.is_none());
    }

    #[test]
    fn parse_struct_empty() {
        let prog = parse("struct Foo {}");
        let struc = prog.struct0();
        assert_eq!(0, struc.fields.len());
        assert_eq!("Foo", struc.name.as_ref().unwrap().name_as_string);
    }

    #[test]
    fn parse_struct_one_field() {
        let prog = parse(
            "struct Bar {
            f1: Foo1,
        }",
        );
        let struc = prog.struct0();
        assert_eq!(1, struc.fields.len());
        assert_eq!("Bar", struc.name.as_ref().unwrap().name_as_string);

        let f1 = &struc.fields[0];
        assert_eq!("f1", f1.name.as_ref().unwrap().name_as_string);
    }

    #[test]
    fn parse_struct_multiple_fields() {
        let prog = parse(
            "struct FooBar {
            fa: Foo1,
            fb: Foo2,
        }",
        );
        let struc = prog.struct0();
        assert_eq!(2, struc.fields.len());
        assert_eq!("FooBar", struc.name.as_ref().unwrap().name_as_string);

        let f1 = &struc.fields[0];
        assert_eq!("fa", f1.name.as_ref().unwrap().name_as_string);

        let f2 = &struc.fields[1];
        assert_eq!("fb", f2.name.as_ref().unwrap().name_as_string);
    }

    #[test]
    fn parse_struct_with_type_params() {
        let prog = parse(
            "struct Bar[T1, T2] {
            f1: T1, f2: T2,
        }",
        );
        let struct_ = prog.struct0();
        assert_eq!(2, struct_.fields.len());
        assert_eq!("Bar", struct_.name.as_ref().unwrap().name_as_string);

        assert_eq!(2, struct_.type_params.as_ref().unwrap().params.len());
    }

    #[test]
    fn parse_struct_literal_while() {
        let expr = parse_expr("while i < n { }");
        let while_expr = expr.to_while().unwrap();
        let bin = while_expr.cond.to_bin().unwrap();

        assert!(bin.lhs.is_sym());
        assert!(bin.rhs.is_sym());
    }

    #[test]
    fn parse_struct_literal_if() {
        let expr = parse_expr("if i < n { }");
        let ifexpr = expr.to_if().unwrap();
        let bin = ifexpr.cond.to_bin().unwrap();

        assert!(bin.lhs.is_sym());
        assert!(bin.rhs.is_sym());
    }

    #[test]
    fn parse_literal_float() {
        let expr = parse_expr("1.2");

        let lit = expr.to_literal_float().unwrap();

        assert_eq!("1.2", lit.value);
    }

    #[test]
    fn parse_template() {
        let expr = parse_expr("\"a${1}b${2}c\"");
        let tmpl = expr.to_template().unwrap();
        assert_eq!(tmpl.parts.len(), 5);

        assert_eq!(
            "\"a${".to_string(),
            tmpl.parts[0].to_literal_str().unwrap().value
        );
        assert_eq!(
            String::from("1"),
            tmpl.parts[1].to_literal_i64().unwrap().value
        );
        assert_eq!(
            "}b${".to_string(),
            tmpl.parts[2].to_literal_str().unwrap().value
        );
        assert_eq!(
            String::from("2"),
            tmpl.parts[3].to_literal_i64().unwrap().value
        );
        assert_eq!(
            "}c\"".to_string(),
            tmpl.parts[4].to_literal_str().unwrap().value
        );

        let expr = parse_expr("\"a\\${1}b\"");
        assert!(expr.is_literal_str());
    }

    #[test]
    fn parse_class_type_params() {
        let prog = parse("class Foo[T]");
        let cls = prog.cls0();

        let type_params = cls.type_params.as_ref().unwrap();
        assert_eq!(1, type_params.params.len());
        assert_eq!(
            "T",
            type_params.params[0].name.as_ref().unwrap().name_as_string
        );

        let prog = parse("class Foo[X]");
        let cls = prog.cls0();

        let type_params = cls.type_params.as_ref().unwrap();
        assert_eq!(1, type_params.params.len());
        assert_eq!(
            "X",
            type_params.params[0].name.as_ref().unwrap().name_as_string
        );
    }

    #[test]
    fn parse_multiple_class_type_params() {
        let prog = parse("class Foo[A, B]");
        let cls = prog.cls0();

        let type_params = cls.type_params.as_ref().unwrap();
        assert_eq!(2, type_params.params.len());
        assert_eq!(
            "A",
            type_params.params[0].name.as_ref().unwrap().name_as_string
        );
        assert_eq!(
            "B",
            type_params.params[1].name.as_ref().unwrap().name_as_string
        );
    }

    #[test]
    fn parse_empty_trait() {
        let prog = parse("trait Foo { }");
        let trait_ = prog.trait0();

        assert_eq!("Foo", trait_.name.as_ref().unwrap().name_as_string);
        assert_eq!(0, trait_.methods.len());
    }

    #[test]
    fn parse_trait_with_function() {
        let prog = parse("trait Foo { fn empty(); }");
        let trait_ = prog.trait0();

        assert_eq!("Foo", trait_.name.as_ref().unwrap().name_as_string);
        assert_eq!(1, trait_.methods.len());
    }

    #[test]
    fn parse_trait_with_static_function() {
        let prog = parse("trait Foo { static fn empty(); }");
        let trait_ = prog.trait0();

        assert_eq!("Foo", trait_.name.as_ref().unwrap().name_as_string);
        assert_eq!(1, trait_.methods.len());
    }

    #[test]
    fn parse_empty_impl() {
        let prog = parse("impl Foo for A {}");
        let impl_ = prog.impl0();

        assert_eq!("Foo", impl_.trait_type.as_ref().unwrap().to_string());
        assert_eq!("A", impl_.extended_type.to_string());
        assert_eq!(0, impl_.methods.len());
    }

    #[test]
    fn parse_impl_with_function() {
        let prog = parse("impl Bar for B { fn foo(); }");
        let impl_ = prog.impl0();

        assert_eq!("Bar", impl_.trait_type.as_ref().unwrap().to_string());
        assert_eq!("B", impl_.extended_type.to_string());
        assert_eq!(1, impl_.methods.len());
    }

    #[test]
    fn parse_impl_with_static_function() {
        let prog = parse("impl Bar for B { static fn foo(); }");
        let impl_ = prog.impl0();

        assert_eq!("Bar", impl_.trait_type.as_ref().unwrap().to_string());
        assert_eq!("B", impl_.extended_type.to_string());
        assert_eq!(1, impl_.methods.len());
    }

    #[test]
    fn parse_global_let() {
        let prog = parse("let b: i64 = 0;");
        let global = prog.global0();

        assert_eq!("b", global.name.as_ref().unwrap().name_as_string);
        assert_eq!(false, global.mutable);
    }

    #[test]
    fn parse_literal_char() {
        let expr = parse_expr("'a'");
        let lit = expr.to_literal_char().unwrap();

        assert_eq!("'a'", lit.value);
    }

    #[test]
    fn parse_function_call_with_type_param() {
        let expr = parse_expr("Array[Int]()");
        let call = expr.to_call().unwrap();
        let type_params = call.callee.to_type_param().unwrap();

        assert_eq!(1, type_params.args.len());

        let expr = parse_expr("Foo[Int, Long]()");
        let call = expr.to_call().unwrap();
        let type_params = call.callee.to_type_param().unwrap();

        assert_eq!(2, type_params.args.len());

        let expr = parse_expr("Bar[]()");
        let call = expr.to_call().unwrap();
        let type_params = call.callee.to_type_param().unwrap();

        assert_eq!(0, type_params.args.len());

        let expr = parse_expr("Vec()");
        let call = expr.to_call().unwrap();

        assert!(call.callee.is_sym());
    }

    #[test]
    fn parse_call_with_path() {
        let expr = parse_expr("Foo::get()");
        let call = expr.to_call().unwrap();

        assert!(call.callee.is_path());
        assert_eq!(0, call.args.len());
    }

    #[test]
    fn parse_function_with_type_params() {
        let prog = parse("fn f[T]() {}");
        let function = prog.function0();

        assert_eq!(1, function.type_params.as_ref().unwrap().params.len());
    }

    #[test]
    fn parse_const() {
        let prog = parse("const x: i64 = 0;");
        let const_ = prog.const0();

        assert_eq!("x", const_.name.as_ref().unwrap().name_as_string);
    }

    #[test]
    fn parse_generic_with_bound() {
        let prog = parse("class A[T: Foo]");
        let cls = prog.cls0();

        let type_param = &cls.type_params.as_ref().unwrap().params[0];
        assert_eq!(1, type_param.bounds.len());
    }

    #[test]
    fn parse_generic_with_multiple_bounds() {
        let prog = parse("class A[T: Foo + Bar]");
        let cls = prog.cls0();

        let type_param = &cls.type_params.as_ref().unwrap().params[0];
        assert_eq!(2, type_param.bounds.len());
    }

    #[test]
    fn parse_lambda_no_params_no_return_value() {
        let expr = parse_expr("|| {}");
        let lambda = expr.to_lambda().unwrap();

        assert!(lambda.return_type.is_none());
    }

    #[test]
    fn parse_lambda_no_params_unit_as_return_value() {
        let expr = parse_expr("|| : () {}");
        let lambda = expr.to_lambda().unwrap();
        let ret = lambda.return_type.as_ref().unwrap();

        assert!(ret.is_unit());
    }

    #[test]
    fn parse_lambda_no_params_with_return_value() {
        let expr = parse_expr("||: A {}");
        let lambda = expr.to_lambda().unwrap();
        let ret = lambda.return_type.as_ref().unwrap();
        let basic = ret.to_basic().unwrap();

        assert_eq!("A", basic.name());
    }

    #[test]
    fn parse_lambda_with_one_param() {
        let expr = parse_expr("|a: A|: B {}");
        let lambda = expr.to_lambda().unwrap();

        assert_eq!(1, lambda.params.len());

        let param = &lambda.params[0];
        assert_eq!("a", param.name.as_ref().unwrap().name_as_string);
        let basic = param.data_type.to_basic().unwrap();
        assert_eq!("A", basic.name());

        let ret = lambda.return_type.as_ref().unwrap();
        let basic = ret.to_basic().unwrap();

        assert_eq!("B", basic.name());
    }

    #[test]
    fn parse_lambda_with_two_params() {
        let expr = parse_expr("|a: A, b: B|: C {}");
        let lambda = expr.to_lambda().unwrap();

        assert_eq!(2, lambda.params.len());

        let param = &lambda.params[0];
        assert_eq!("a", param.name.as_ref().unwrap().name_as_string);
        let basic = param.data_type.to_basic().unwrap();
        assert_eq!("A", basic.name());

        let param = &lambda.params[1];
        assert_eq!("b", param.name.as_ref().unwrap().name_as_string);
        let basic = param.data_type.to_basic().unwrap();
        assert_eq!("B", basic.name());

        let ret = lambda.return_type.as_ref().unwrap();
        let basic = ret.to_basic().unwrap();

        assert_eq!("C", basic.name());
    }

    #[test]
    fn parse_for() {
        let expr = parse_expr("for i in a+b {}");
        assert!(expr.is_for());
    }

    #[test]
    fn parse_new_call_sym() {
        let expr = parse_expr("i");
        assert!(expr.is_sym());
    }

    #[test]
    fn parse_new_call_path() {
        let expr = parse_expr("Foo::bar");
        let path = expr.to_path().unwrap();
        assert!(path.lhs.is_sym());
        assert!(path.rhs.is_sym());
    }

    #[test]
    fn parse_new_call_call() {
        let expr = parse_expr("foo(1,2)");
        let call = expr.to_call().unwrap();
        assert!(call.callee.is_sym());
        assert_eq!(call.args.len(), 2);
    }

    #[test]
    fn parse_block() {
        let expr = parse_expr("{1}");
        assert!(expr
            .to_block()
            .unwrap()
            .expr
            .as_ref()
            .unwrap()
            .is_literal_i64());

        let expr = parse_expr("({}) + 1");
        assert!(expr.is_bin());

        let expr = parse_expr("1 + {}");
        assert!(expr.is_bin());
    }

    #[test]
    fn parse_tuple() {
        let expr = parse_expr("(1,)");
        assert_eq!(expr.to_tuple().unwrap().values.len(), 1);

        let expr = parse_expr("(1)");
        assert!(expr.is_paren());

        let expr = parse_expr("(1,2,3)");
        assert_eq!(expr.to_tuple().unwrap().values.len(), 3);

        let expr = parse_expr("(1,2,3,4,)");
        assert_eq!(expr.to_tuple().unwrap().values.len(), 4);
    }

    #[test]
    fn parse_enum() {
        let prog = parse("enum Foo { A, B, C }");
        let enum_ = prog.enum0();
        assert_eq!(enum_.variants.len(), 3);
    }

    #[test]
    fn parse_enum_with_type_params() {
        let prog = parse("enum MyOption[T] { None, Some(T), }");
        let enum_ = prog.enum0();
        assert_eq!(enum_.variants.len(), 2);
        assert!(enum_.variants[0].types.is_none());
        assert_eq!(enum_.variants[1].types.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn parse_module() {
        let prog = parse("mod foo { fn bar() {} fn baz() {} }");
        let module = prog.module0();
        let elements = module.elements.as_ref().unwrap();
        assert_eq!(elements.len(), 2);
        assert!(elements[0].to_function().is_some());
        assert!(elements[1].to_function().is_some());
    }

    #[test]
    fn parse_mod_without_body() {
        let prog = parse("mod foo;");
        let module = prog.module0();
        assert!(module.elements.is_none());
    }

    #[test]
    fn parse_match() {
        parse_expr("match x { }");
        parse_expr("match x { A(x, b) => 1, B => 2 }");
        parse_expr("match x { A(x, b) => 1, B | C => 2 }");
    }

    #[test]
    fn parse_type_alias_in_trait() {
        parse(
            "trait Foo {
            type MY_TYPE;
        }",
        );
    }

    #[test]
    fn parse_where_clauses() {
        parse(
            "
            fn f() where A: B {}
            struct F where A: B, C: D {}
            class F where A: B + C, D: E {}
            impl F for X where A: B + C + D, E: F {}
            trait F where A: B {}
            enum F where A: B + C { A, B }
        ",
        );
    }
}
