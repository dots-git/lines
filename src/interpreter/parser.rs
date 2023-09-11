use super::debug::{DebugInfo, FileOrOtherError, Error};
use super::iter::{CanConcatenate, SingleItemIterator};
use super::lexer::{DebugToken, Lexer, Token};
use crate::file_handler;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::vec;

macro_rules! expect_token {
    ($self:expr, $token_source:expr, $($pattern:pat => $result:expr),* $(,)?; $msg:expr) => {
        match $token_source {
            $(
                Some($pattern) => $result,
            )*
            Some(t) => {
                let token_str = t.simple_string();
                return Err(Error::UnexpectedTokenError(
                    $self.get_error_prefix(),
                    token_str,
                    $msg.to_string(),
                ));
            }
            _ => {
                return Err(Error::UnexpectedEndOfFileError(
                    $self.get_error_prefix(),
                    $msg.to_string(),
                ));
            }
        }
    };
}

macro_rules! expect_any_token {
    ($self:expr, $token_source:expr, $($pattern:pat => $result:expr),* $(,)?; $msg:expr) => {
        match $token_source {
            $(
                Some($pattern) => $result,
            )*
            _ => {
                return Err(Error::UnexpectedEndOfFileError(
                    $self.get_error_prefix(),
                    $msg.to_string(),
                ));
            }
        }
    };
}

lazy_static! {
    static ref LITERAL_PREFIXES: HashMap<String, u32> = {
        let mut set = HashMap::new();
        set.insert("bin_".into(), 2);
        set.insert("dec_".into(), 10);
        set.insert("hex_".into(), 16);
        set.insert("#".into(), 16);
        set
    };
}

#[derive(Debug, Clone)]
pub struct CodeBody {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Literal {
    String(String),
    Int(i64),
    Float(f64),
}

#[derive(Debug)]
pub enum BinaryOperator {
    Add(Expression, Expression),
    Sub(Expression, Expression),
    Mul(Expression, Expression),
    Div(Expression, Expression),
}

pub enum BinaryOperatorType {
    Add,
    Sub,
    Mul,
    Div,
}

impl BinaryOperator {
    pub fn get_precedence(token: &BinaryOperatorType) -> usize {
        match token {
            BinaryOperatorType::Add => 1,
            BinaryOperatorType::Sub => 1,
            BinaryOperatorType::Mul => 2,
            BinaryOperatorType::Div => 2,
        }
    }

    pub fn get_operator(
        token: &BinaryOperatorType,
        a: Expression,
        b: Expression,
    ) -> BinaryOperator {
        match token {
            BinaryOperatorType::Add => BinaryOperator::Add(a, b),
            BinaryOperatorType::Sub => BinaryOperator::Sub(a, b),
            BinaryOperatorType::Mul => BinaryOperator::Mul(a, b),
            BinaryOperatorType::Div => BinaryOperator::Div(a, b),
        }
    }

    pub fn get_operator_type(token: &Token) -> Option<BinaryOperatorType> {
        match token {
            Token::Plus => Some(BinaryOperatorType::Add),
            Token::Minus => Some(BinaryOperatorType::Sub),
            Token::Star => Some(BinaryOperatorType::Mul),
            Token::Slash => Some(BinaryOperatorType::Div),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct FunctionCall {
    pub expr: Expression,
    pub args: Vec<Expression>,
    pub kwargs: HashMap<String, Expression>,
}

#[derive(Debug, Clone)]
pub enum Indentifier {
    Final(String, Range<usize>),
    Member(String, Rc<Expression>, Range<usize>),
}

#[derive(Debug, Clone)]
pub enum Expression {
    Literal(Literal),
    Identifier(Indentifier),
    BinaryOperator(Rc<BinaryOperator>),
    FunctionCall(Rc<FunctionCall>),
}

pub trait DebugString {
    fn debug_string(&self) -> String;
}

impl DebugString for Expression {
    fn debug_string(&self) -> String {
        match self {
            Expression::Identifier(i) => i.debug_string(),
            _ => format!("{:?}", self),
        }
    }
}

impl DebugString for Indentifier {
    fn debug_string(&self) -> String {
        match self {
            Indentifier::Member(name, member, _) => {
                format!("{}.{:?}", name, member)
            }
            Indentifier::Final(name, _) => format!("{}", name),
        }
    }
}

pub trait GetCharRange {
    fn get_char_range(&self) -> Range<usize>;
}

impl GetCharRange for Expression {
    fn get_char_range(&self) -> Range<usize> {
        match self {
            Expression::Identifier(i) => return i.get_char_range(),
            Expression::Literal(l) => l.get_char_range(),
            Expression::BinaryOperator(b) => b.get_char_range(),
            Expression::FunctionCall(f) => f.get_char_range(),
        }
    }
}

impl GetCharRange for Indentifier {
    fn get_char_range(&self) -> Range<usize> {
        match self {
            Indentifier::Final(_, range) => range.clone(),
            Indentifier::Member(_, _, range) => range.clone(),
        }
    }
}

impl GetCharRange for Literal {
    fn get_char_range(&self) -> Range<usize> {
        1..0
    }
}

impl GetCharRange for BinaryOperator {
    fn get_char_range(&self) -> Range<usize> {
        1..0
    }
}

impl GetCharRange for FunctionCall {
    fn get_char_range(&self) -> Range<usize> {
        1..0
    }
}

impl From<Literal> for Expression {
    fn from(value: Literal) -> Self {
        Expression::Literal(value)
    }
}
impl From<BinaryOperator> for Expression {
    fn from(value: BinaryOperator) -> Self {
        Expression::BinaryOperator(Rc::new(value))
    }
}

impl From<FunctionCall> for Expression {
    fn from(value: FunctionCall) -> Self {
        Expression::FunctionCall(Rc::new(value))
    }
}

impl From<Indentifier> for Expression {
    fn from(value: Indentifier) -> Self {
        Expression::Identifier(value)
    }
}

#[derive(Debug, Clone)]
pub struct Assignment {
    // WORD(name) EQUALS <Expression>(value)
    pub name: String,
    pub value: Expression,
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub args: Vec<String>,
    pub body: CodeBody,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Expression(Expression),
    Assignment(Assignment),
    FunctionDefinition(FunctionDefinition),
}

impl From<Expression> for Statement {
    fn from(value: Expression) -> Self {
        Statement::Expression(value)
    }
}

impl From<Assignment> for Statement {
    fn from(value: Assignment) -> Self {
        Statement::Assignment(value)
    }
}

impl From<FunctionDefinition> for Statement {
    fn from(value: FunctionDefinition) -> Self {
        Statement::FunctionDefinition(value)
    }
}

trait IsNumeric {
    fn is_numeric(&self) -> bool;
}

impl IsNumeric for str {
    fn is_numeric(&self) -> bool {
        for char in self.chars() {
            if !char.is_numeric() {
                return false;
            }
        }
        return true;
    }
}

trait CanBeInt {
    fn can_be_int(&self) -> bool;
}

impl CanBeInt for f64 {
    fn can_be_int(&self) -> bool {
        self.fract() == 0.0 && self >= &(i64::MIN as f64) && self <= &(i64::MAX as f64)
    }
}

pub struct Parser {
    debug_info: DebugInfo,
    tokens: Vec<DebugToken>,
    pointer: usize,
    paren_depth: usize,
}

impl Parser {
    pub fn parse_tokens(
        tokens: Vec<DebugToken>,
        debug_info: Option<DebugInfo>,
    ) -> Result<CodeBody, Error> {
        let mut instance: Parser = Self {
            debug_info: match debug_info {
                Some(d) => d,
                None => DebugInfo::new(None, "(unknown source)".to_string()),
            },
            tokens: tokens,
            pointer: 0,
            paren_depth: 0,
        };
        instance.get_tree()
    }

    #[allow(dead_code)]
    pub fn parse_string(
        s: String,
        debug_info: Option<DebugInfo>,
    ) -> Result<CodeBody, Error> {
        let tokens = Lexer::lex_string(s.to_string(), debug_info.clone())?;
        let result: Result<CodeBody, Error> = Self::parse_tokens(tokens, debug_info);
        result
    }

    pub fn parse_file(path: String) -> Result<CodeBody, FileOrOtherError> {
        let file_content = match file_handler::get_file_contents(&path) {
            Ok(contents) => contents,
            Err(err) => return Err(FileOrOtherError::IOError(err)),
        };
        let tokens: Vec<DebugToken> = Lexer::lex_file(path.to_string())?;
        let result = match Self::parse_tokens(
            tokens,
            Some(DebugInfo::new(Some(file_content), format!("\"{}\"", path))),
        ) {
            Ok(v) => Ok(v),
            Err(err) => Err(FileOrOtherError::OtherError(err)),
        };
        result
    }

    pub fn get_tree(&mut self) -> Result<CodeBody, Error> {
        match self.peek_token() {
            Some(Token::NewLine(n)) => {
                if n.indent != 0 {
                    return Err(Error::UnexpectedIndentError(
                        self.get_error_prefix(),
                    ));
                }
            }
            None => return Ok(CodeBody { statements: vec![] }),
            Some(t) => {
                let token_str = t.simple_string();
                return Err(Error::UnexpectedTokenError(
                    self.get_error_prefix(),
                    token_str,
                    "the first token must be a newline token with 0 indent.".into(),
                ));
            }
        }
        let indent: usize = match self.next_token() {
            Some(Token::NewLine(s)) => s.indent,
            Some(_) => 0,
            None => return Ok(CodeBody { statements: vec![] }),
        };
        self.next_code_body(indent)
    }

    fn next_code_body(&mut self, indent: usize) -> Result<CodeBody, Error> {
        let mut statements = vec![];

        loop {
            let this_statement = self.next_statement(indent)?;
            match this_statement {
                None => break,
                Some(statement) => statements.push(statement),
            }
            match self.peek_token() {
                Some(Token::NewLine(n)) => {
                    if n.indent < indent {
                        break;
                    };
                    if n.indent > indent {
                        self.move_n(2);
                        return Err(Error::UnexpectedIndentError(
                            self.get_error_prefix(),
                        ));
                    };
                }
                Some(t) => {
                    let token_str = t.simple_string();
                    self.move_n(1);
                    return Err(Error::UnexpectedTokenError(
                        self.get_error_prefix(),
                        token_str,
                        "expected the beginning of a new line.".to_string(),
                    ));
                }
                None => break,
            }
        }

        Ok(CodeBody {
            statements: statements,
        })
    }

    fn next_statement(&mut self, indent: usize) -> Result<Option<Statement>, Error> {
        while let Some(Token::NewLine(_)) = self.peek_token() {
            self.next();
        }
        let next_token_option = self.peek_token();
        let next_token = match next_token_option {
            None => return Ok(None),
            Some(t) => t,
        };

        if let Token::Word(first_word) = next_token {
            // check for function definition pattern "WORD(def) WORD PAREN_OPEN"
            if first_word == "def" {
                if let (Some(Token::Word(name)), Some(Token::ParenOpen)) =
                    (self.peek_n_token(1), self.peek_n_token(2))
                {
                    let name = name.to_string();
                    self.move_n(3);

                    expect_token!(
                        self, self.next_token(),
                        Token::ParenClose => ();
                        "expected closing parenthesis."
                    );

                    expect_token!(
                        self, self.next_token(),
                        Token::Colon => ();
                        "expected colon."
                    );

                    let mut next_indent = expect_token!(
                        self, self.next_token(),
                        Token::NewLine(n) => n;
                        "expected new line."
                    )
                    .indent;

                    while let Some(Token::NewLine(n)) = self.peek_token() {
                        next_indent = n.indent;
                        self.next();
                    }

                    if next_indent <= indent {
                        return Err(Error::ExpectedIndentError(
                            self.get_error_prefix(),
                            "expected indented block for function declaration.".into(),
                        ));
                    }
                    let body = self.next_code_body(next_indent)?;

                    return Ok(Some(Statement::from(FunctionDefinition {
                        name: name,
                        args: vec![],
                        body: body,
                    })));
                }
                self.next();
                return Err(Error::MissingTokenError(
                    self.get_error_prefix(),
                    "expected function definition after keyword \"def\"".into(),
                ));
            }
        }

        if let Token::Word(first_word) = next_token {
            // check for assignment pattern "WORD EQUALS ..."
            let word = first_word.to_string();
            if let Some(Token::Equals) = self.peek_n_token(1) {
                return Ok(Some(Statement::from(self.next_assignment(word)?)));
            }
        }

        return match self.next_expression(0)? {
            Some(expr) => Ok(Some(Statement::from(expr))),
            _ => Err(Error::UnexpectedEndOfStatementError(
                self.get_error_prefix(),
                "expected a statement.".into(),
            )),
        };
    }

    fn next_assignment(&mut self, identifier: String) -> Result<Assignment, Error> {
        self.next();
        self.next();
        let expression = match self.next_expression(0)? {
            Some(e) => e,
            None => {
                return Err(Error::ExpectedExpressionError(
                    self.get_error_prefix(),
                ))
            }
        };
        return Ok(Assignment {
            name: identifier,
            value: expression,
        });
    }

    fn next_expression(&mut self, min_prec: usize) -> Result<Option<Expression>, Error> {
        let mut a = match self.next_atomic_expression()? {
            Some(a) => a,
            None => return Ok(None),
        };

        loop {
            match self.peek_token() {
                Some(Token::ParenOpen) => {
                    if let Some(Token::ParenOpen) = self.peek_token() {
                        a = self.next_function_call(a)?;
                    }
                }
                Some(Token::Period) => {
                    a = self.next_member(a)?;
                }
                Some(Token::ParenClose) => {
                    if self.paren_depth > 0 {
                        return Ok(Some(a));
                    } else {
                        return Err(Error::UnexpectedTokenError(
                            self.get_error_prefix(),
                            Token::ParenClose.simple_string(),
                            "".into(),
                        ));
                    }
                }
                Some(Token::NewLine(_)) => return Ok(Some(a)),
                None => return Ok(Some(a)),
                Some(_) => break,
            };
        }
        // if the next token is a binary operator with a precedence higher or equal to min_prec, then:
        //     consume and save it
        //     continue by evaluating the next expression
        loop {
            let next = match self.peek_token() {
                Some(token) => token,
                None => break,
            };
            let op_type = match BinaryOperator::get_operator_type(next) {
                Some(op_type) => op_type,
                None => break,
            };
            let op_prec = BinaryOperator::get_precedence(&op_type);
            if op_prec < min_prec {
                break;
            }
            self.next();

            a = match self.next_expression(op_prec + 1)? {
                Some(b) => Expression::from(BinaryOperator::get_operator(&op_type, a, b)),
                None => {
                    return Err(Error::UnexpectedEndOfStatementError(
                        self.get_error_prefix(),
                        format!(
                            "expected expression after operator {}",
                            self.prev_token().unwrap().simple_string()
                        ),
                    ))
                }
            };
        }

        Ok(Some(a))
    }

    fn next_function_call(&mut self, a: Expression) -> Result<Expression, Error> {
        let mut all_args = vec![];
        self.paren_depth += 1;
        loop {
            self.next();
            expect_any_token!(self, self.peek_token(),
                Token::ParenClose => break,
                _ => ();
                "expected expression or closing parentheses."
            );
            let expr = match self.next_expression(0)? {
                Some(expr) => expr,
                None => {
                    return Err(Error::ExpectedExpressionError(
                        self.get_error_prefix(),
                    ))
                }
            };
            all_args.push(expr);
            expect_token!(self, self.peek_token(),
                Token::Comma => {
                    continue;
                },
                Token::ParenClose => break;
                "expected comma or closing parentheses."
            )
        }
        self.paren_depth -= 1;
        self.next();
        return Ok(Expression::from(FunctionCall {
            expr: a,
            args: all_args,
            kwargs: HashMap::new(),
        }));
    }

    fn next_member(&mut self, a: Expression) -> Result<Expression, Error> {
        self.next();
        let next = match self.peek() {
            Some(t) => t,
            None => {
                return Err(Error::UnexpectedEndOfFileError(
                    self.get_error_prefix(),
                    "expected a member identifier.".into(),
                ))
            }
        };
        match &next.token {
            Token::Word(w) => {
                self.assert_not_literal(&w)?;
                let name = w.to_string();
                let range = next.char_range.clone();
                self.next();
                return Ok(Expression::Identifier(Indentifier::Member(
                    name,
                    Rc::new(a),
                    range,
                )));
            }
            _ => Err(Error::UnexpectedTokenError(
                self.get_error_prefix(),
                next.token.simple_string(),
                "expected a member identifier.".into(),
            )),
        }
    }

    fn next_atomic_expression(&mut self) -> Result<Option<Expression>, Error> {
        match self.next_token() {
            Some(Token::Word(w)) => {
                let word = w.to_string();
                return self.next_word(word);
            }
            Some(Token::StringLiteral(w)) => {
                let string = w.to_string();
                Ok(Some(Expression::from(Literal::String(string))))
            }
            Some(Token::ParenOpen) => {
                self.paren_depth += 1;
                let rv = self.next_expression(0);
                self.paren_depth -= 1;
                self.next();
                rv
            }
            _ => {
                return Err(Error::UnexpectedEndOfStatementError(
                    self.get_error_prefix(),
                    "".into(),
                ));
            }
        }
    }

    fn next_word(&mut self, word: String) -> Result<Option<Expression>, Error> {
        if let Ok(value) = word.parse() {
            if let Some(Token::Period) = self.peek_token() {
                self.next();
                if let Some(Token::Word(frac)) = self.next_token() {
                    let mut full_str = word;
                    full_str.push('.');
                    full_str.push_str(frac);
                    return match full_str.parse() {
                        Ok(value) => Ok(Some(Expression::from(Literal::Float(value)))),
                        Err(_) => Err(Error::InvalidLiteralError(
                            self.get_error_prefix(),
                            full_str,
                            "expected this to be a float literal.".into(),
                        )),
                    };
                }
            }
            return Ok(Some(Expression::from(Literal::Int(value))));
        }

        if word.ends_with("e") && word[..word.len() - 1].is_numeric() {
            if let Some(Token::Word(w)) = self.peek_n_token(1) {
                let operator = match self.peek_n_token(0) {
                    Some(Token::Plus) => Some('+'),
                    Some(Token::Minus) => Some('-'),
                    _ => None,
                };
                match operator {
                    Some(operator) => {
                        let mut full_value = word.to_string();
                        full_value.push(operator);
                        full_value.push_str(&w);
                        if let Ok(value) = full_value.parse::<f64>() {
                            self.next();
                            self.next();
                            if value.can_be_int() {
                                return Ok(Some(Expression::from(Literal::Int(value as i64))));
                            }
                            return Ok(Some(Expression::from(Literal::Float(value))));
                        }
                    }
                    None => (),
                }
            };
            return Err(Error::InvalidLiteralError(
                self.get_error_prefix(),
                word,
                "expected float literal due to numeric followed by \"e\". example: 10e-3".into(),
            ));
        }

        for k in LITERAL_PREFIXES.keys() {
            if !word.starts_with(k) {
                continue;
            }
            let rest_word = &word[k.len()..];
            if let Ok(value) = i64::from_str_radix(rest_word, LITERAL_PREFIXES[k]) {
                return Ok(Some(Expression::from(Literal::Int(value))));
            }
            return Err(Error::InvalidLiteralError(
                self.get_error_prefix(),
                format!("{:?}", word),
                format!(
                    "expected an int literal with base {} after prefix \"{}\"",
                    LITERAL_PREFIXES[k], k
                )
                .into(),
            ));
        }

        if word.starts_with("base_") {
            let mut remaining_word_iter: std::str::Split<'_, &str> = (&word[5..]).split("_");
            if let Some(base_str) = remaining_word_iter.next() {
                if let Ok(base) = base_str.parse() {
                    if let Some(literal) = remaining_word_iter.next() {
                        if let Ok(value) = i64::from_str_radix(literal, base) {
                            return Ok(Some(Expression::from(Literal::Int(value))));
                        }
                    }
                }
            }
            return Err(Error::InvalidLiteralError(
                self.get_error_prefix(),
                word,
                "expected a base n and an int literal with base n after prefix \"base_\". example: \"base_3_2102\"".into()
            ));
        }

        // let prev_debug_token = self.prev().unwrap();
        // let prev_char_range = prev_debug_token.char_range.start..prev_debug_token.char_range.end;
        // let mut members = vec![(word, prev_char_range)];

        // while let Some(Token::Period) = self.peek_token() {
        //     self.next();
        //     let next_token = match self.next() {
        //         Some(t) => t,
        //         _ => continue,
        //     };
        //     match &next_token.token {
        //         Token::Word(w) => members.push((
        //             w.to_string(),
        //             next_token.char_range.start..next_token.char_range.end,
        //         )),
        //         t => {
        //             let token_string = t.simple_string();
        //             return Err(InterpreterError::UnexpectedTokenError(
        //                 self.get_error_prefix(),
        //                 token_string,
        //                 "expected member identifier.".into(),
        //             ));
        //         }
        //     }
        // }

        // let last_identifier = members.pop().unwrap();
        // self.assert_not_literal(&last_identifier.0)?;
        // let mut variable = Indentifier::Final(last_identifier.0.to_string(), last_identifier.1);
        // for (identifier, char_range) in members.iter().rev() {
        //     self.assert_not_literal(identifier)?;
        //     variable = Indentifier::MemberOf(
        //         identifier.to_string(),
        //         Rc::new(variable),
        //         char_range.start..char_range.end,
        //     );
        // }

        Ok(Some(Expression::from(Indentifier::Final(
            word,
            self.prev().unwrap().char_range.clone(),
        ))))
    }

    fn assert_not_literal(&self, identifier: &str) -> Result<(), Error> {
        if identifier
            .chars()
            .nth(0)
            .expect("zero-size identifier")
            .is_numeric()
        {
            return Err(Error::UnexpectedLiteralError(
                self.get_error_prefix(),
                identifier.into(),
                "a literal cannot be placed in member syntax (var.member). was this meant to be a float literal?".into()
            ));
        }
        for prefix in LITERAL_PREFIXES
            .keys()
            .concat(SingleItemIterator::new(&"base_".to_string()))
        {
            if identifier.starts_with(prefix) {
                return Err(Error::UnexpectedLiteralError(
                    self.get_error_prefix(),
                    identifier.into(),
                    format!("a literal cannot be placed in member syntax (var.member). tokens starting with {} are interpreted as numeric literals.", prefix)
                ));
            }
        }
        Ok(())
    }

    fn skip_newline_back(&self, mut pointer: usize) -> usize {
        loop {
            let token = match self.tokens.get(pointer) {
                Some(t) => t,
                None => break,
            };
            match token.token {
                Token::NewLine(_) => {
                    if pointer > 0 {
                        pointer -= 1;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        pointer
    }

    fn skip_newline_forward(&self, mut pointer: usize) -> usize {
        loop {
            let token = match self.tokens.get(pointer) {
                Some(t) => t,
                None => break,
            };
            match token.token {
                Token::NewLine(_) => pointer += 1,
                _ => break,
            }
        }
        pointer
    }

    fn prev(&self) -> Option<&DebugToken> {
        let mut temp_pointer = self.pointer - 1;

        if self.paren_depth > 0 {
            temp_pointer = self.skip_newline_back(temp_pointer);
        }

        self.tokens.get(temp_pointer)
    }

    fn peek(&self) -> Option<&DebugToken> {
        let mut temp_pointer = self.pointer;
        if self.paren_depth > 0 {
            temp_pointer = self.skip_newline_back(temp_pointer);
        }
        self.tokens.get(temp_pointer)
    }

    fn _peek_n(&self, n: usize) -> Option<&DebugToken> {
        let mut temp_pointer = self.pointer;
        if self.paren_depth > 0 {
            for _ in 0..n {
                temp_pointer = self.skip_newline_forward(temp_pointer + 1);
            }
        } else {
            temp_pointer += n;
        }

        self.tokens.get(temp_pointer)
    }

    fn next(&mut self) -> Option<&DebugToken> {
        let rv = self.tokens.get(self.pointer);

        self.pointer += 1;
        if self.paren_depth > 0 {
            self.pointer = self.skip_newline_forward(self.pointer);
        }

        rv
    }

    fn move_n(&mut self, n: usize) -> () {
        if self.paren_depth > 0 {
            for _ in 0..n {
                self.pointer = self.skip_newline_forward(self.pointer + 1);
            }
        } else {
            self.pointer += n;
        }
    }

    fn prev_token(&self) -> Option<&Token> {
        self.get_token(self.pointer - 1)
    }

    fn peek_token(&self) -> Option<&Token> {
        self.get_token(self.pointer)
    }

    fn peek_n_token(&self, n: usize) -> Option<&Token> {
        let mut temp_pointer = self.pointer;
        if self.paren_depth > 0 {
            for _ in 0..n {
                temp_pointer = self.skip_newline_forward(temp_pointer + 1);
            }
        } else {
            temp_pointer += n;
        }
        self.get_token(temp_pointer)
    }

    fn next_token(&mut self) -> Option<&Token> {
        let rv = match self.tokens.get(self.pointer) {
            Some(token) => Some(&token.token),
            None => None,
        };

        self.pointer += 1;
        if self.paren_depth > 0 {
            self.pointer = self.skip_newline_forward(self.pointer);
        }

        rv
    }

    fn get_token(&self, index: usize) -> Option<&Token> {
        match self.tokens.get(index) {
            Some(token) => Some(&token.token),
            None => None,
        }
    }

    fn _print_next_n(&mut self, n: usize) {
        println!("printing next {} tokens", n);
        for i in 0..n {
            println!("{:?}", self._peek_n(i))
        }
    }

    fn get_error_prefix(&self) -> String {
        if let None = self.debug_info.source {
            return "".to_string();
        }

        let mut line = 0;
        let mut column = 0;
        let s: &String = &self
            .debug_info
            .source
            .as_ref()
            .expect("suddenly there was no string");
        let prev_debug_token = if self.pointer == 0 {
            self.peek()
        } else {
            self.prev()
        };
        let iter: Box<dyn Iterator<Item = char>> = match prev_debug_token {
            Some(t) => Box::new(s.chars().take(t.char_range.start)),
            None => Box::new(s.chars()),
        };
        for char in iter {
            column += 1;
            if char == '\n' {
                line += 1;
                column = 0;
            }
        }

        format!(
            "in {}, line {}, column {}:",
            self.debug_info.source_name,
            line + 1,
            column + 1,
        )
    }
}
