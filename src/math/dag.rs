use std::collections::{HashMap, HashSet};
use std::fmt;
use std::mem::discriminant;

use super::lexer::{Token, TokenType};

pub type NodeId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Sin,
    Cos,
    Tan,
    Sqrt,
    Abs,
    Ln,
    Log10,
    Exp,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Neg => "Neg",
            Self::Sin => "Sin",
            Self::Cos => "Cos",
            Self::Tan => "Tan",
            Self::Sqrt => "Sqrt",
            Self::Abs => "Abs",
            Self::Ln => "Ln",
            Self::Log10 => "Log10",
            Self::Exp => "Exp",
        };

        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Min,
    Max,
    Log,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Add => "Add",
            Self::Sub => "Sub",
            Self::Mul => "Mul",
            Self::Div => "Div",
            Self::Rem => "Rem",
            Self::Pow => "Pow",
            Self::Min => "Min",
            Self::Max => "Max",
            Self::Log => "Log",
        };

        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Constant(f64),
    Variable(String),

    Unary {
        op: UnaryOp,
        input: NodeId,
    },

    Binary {
        op: BinaryOp,
        lhs: NodeId,
        rhs: NodeId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
}

#[derive(Debug, Clone)]
pub struct Graph {
    nodes: Vec<Node>,
    root: NodeId,
}

impl Graph {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn dependencies(&self, id: NodeId) -> Option<Vec<NodeId>> {
        let node = self.node(id)?;

        Some(match &node.kind {
            NodeKind::Constant(_) | NodeKind::Variable(_) => Vec::new(),

            NodeKind::Unary { input, .. } => {
                vec![*input]
            }

            NodeKind::Binary { lhs, rhs, .. } => {
                vec![*lhs, *rhs]
            }
        })
    }

    pub fn topological_order(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();

        // (node_id, expanded)
        let mut stack = vec![(self.root, false)];

        while let Some((id, expanded)) = stack.pop() {
            if expanded {
                result.push(id);
                continue;
            }

            if !visited.insert(id) {
                continue;
            }

            stack.push((id, true));

            if let Some(dependencies) = self.dependencies(id) {
                for dependency in dependencies.into_iter().rev() {
                    if !visited.contains(&dependency) {
                        stack.push((dependency, false));
                    }
                }
            }
        }

        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NodeKey {
    Unary {
        op: UnaryOp,
        input: NodeId,
    },

    Binary {
        op: BinaryOp,
        lhs: NodeId,
        rhs: NodeId,
    },
}

#[derive(Debug, Default)]
pub struct GraphBuilder {
    nodes: Vec<Node>,
    variables: HashMap<String, NodeId>,
    constants: HashMap<u64, NodeId>,
    expressions: HashMap<NodeKey, NodeId>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, kind: NodeKind) -> NodeId {
        let id = self.nodes.len();

        self.nodes.push(Node { id, kind });

        id
    }

    pub fn constant(&mut self, value: f64) -> NodeId {
        let key = value.to_bits();

        if let Some(&id) = self.constants.get(&key) {
            return id;
        }

        let id = self.push(NodeKind::Constant(value));

        self.constants.insert(key, id);

        id
    }

    fn constant_value(&self, id: NodeId) -> Option<f64> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Constant(value) => Some(*value),
            _ => None,
        }
    }

    pub fn variable(&mut self, name: impl Into<String>) -> NodeId {
        let name = name.into();

        if let Some(&id) = self.variables.get(&name) {
            return id;
        }

        let id = self.push(NodeKind::Variable(name.clone()));

        self.variables.insert(name, id);

        id
    }

    pub fn unary(&mut self, op: UnaryOp, input: NodeId) -> NodeId {
        assert!(input < self.nodes.len());

        if let Some(value) = self.constant_value(input) {
            let result = match op {
                UnaryOp::Neg => -value,
                UnaryOp::Sin => value.sin(),
                UnaryOp::Cos => value.cos(),
                UnaryOp::Tan => value.tan(),
                UnaryOp::Sqrt => value.sqrt(),
                UnaryOp::Abs => value.abs(),
                UnaryOp::Ln => value.ln(),
                UnaryOp::Log10 => value.log10(),
                UnaryOp::Exp => value.exp(),
            };

            return self.constant(result);
        }

        // -(-x) => x
        if op == UnaryOp::Neg {
            if let NodeKind::Unary {
                op: UnaryOp::Neg,
                input: inner,
            } = &self.nodes[input].kind
            {
                return *inner;
            }
        }

        let key = NodeKey::Unary { op, input };

        if let Some(&id) = self.expressions.get(&key) {
            return id;
        }

        let id = self.push(NodeKind::Unary { op, input });

        self.expressions.insert(key, id);

        id
    }

    pub fn binary(&mut self, op: BinaryOp, lhs: NodeId, rhs: NodeId) -> NodeId {
        assert!(lhs < self.nodes.len());
        assert!(rhs < self.nodes.len());

        let lhs_value = self.constant_value(lhs);
        let rhs_value = self.constant_value(rhs);

        if let (Some(a), Some(b)) = (lhs_value, rhs_value) {
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => a / b,
                BinaryOp::Rem => a % b,
                BinaryOp::Pow => a.powf(b),
                BinaryOp::Min => a.min(b),
                BinaryOp::Max => a.max(b),
                BinaryOp::Log => a.log(b),
            };

            return self.constant(result);
        }

        match op {
            BinaryOp::Add => {
                if rhs_value == Some(0.0) {
                    return lhs;
                }

                if lhs_value == Some(0.0) {
                    return rhs;
                }
            }

            BinaryOp::Sub => {
                if rhs_value == Some(0.0) {
                    return lhs;
                }
            }

            BinaryOp::Mul => {
                if rhs_value == Some(1.0) {
                    return lhs;
                }

                if lhs_value == Some(1.0) {
                    return rhs;
                }
            }

            BinaryOp::Div => {
                if rhs_value == Some(1.0) {
                    return lhs;
                }
            }

            BinaryOp::Pow => {
                if rhs_value == Some(1.0) {
                    return lhs;
                }
            }

            _ => {}
        }

        let (key_lhs, key_rhs) = match op {
            BinaryOp::Add | BinaryOp::Mul | BinaryOp::Min | BinaryOp::Max => {
                if lhs <= rhs {
                    (lhs, rhs)
                } else {
                    (rhs, lhs)
                }
            }

            _ => (lhs, rhs),
        };

        let key = NodeKey::Binary {
            op,
            lhs: key_lhs,
            rhs: key_rhs,
        };

        if let Some(&id) = self.expressions.get(&key) {
            return id;
        }

        let id = self.push(NodeKind::Binary { op, lhs, rhs });

        self.expressions.insert(key, id);

        id
    }

    pub fn finish(self, root: NodeId) -> Graph {
        assert!(root < self.nodes.len());

        Graph {
            nodes: self.nodes,
            root,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at position {}", self.message, self.position)
    }
}

impl std::error::Error for ParseError {}

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    builder: GraphBuilder,
    absolute_depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
            builder: GraphBuilder::new(),
            absolute_depth: 0,
        }
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn current_type(&self) -> Option<&TokenType> {
        self.current().map(|token| &token.token_type)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();

        if token.is_some() {
            self.position += 1;
        }

        token
    }

    fn check(&self, expected: &TokenType) -> bool {
        self.current_type()
            .map(|current| discriminant(current) == discriminant(expected))
            .unwrap_or(false)
    }

    fn expect(&mut self, expected: TokenType) -> Result<Token, ParseError> {
        if self.check(&expected) {
            return Ok(self.advance().unwrap());
        }

        Err(self.error(format!(
            "Expected {:?}, found {:?}",
            expected,
            self.current_type()
        )))
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        let position = self
            .current()
            .map(|token| token.start)
            .or_else(|| self.tokens.last().map(|token| token.end))
            .unwrap_or(0);

        ParseError {
            message: message.into(),
            position,
        }
    }

    pub fn parse(mut self) -> Result<Graph, ParseError> {
        if self.tokens.is_empty() {
            return Err(self.error("Empty expression"));
        }

        let root = self.parse_expression(0)?;

        if !self.check(&TokenType::EOF) {
            return Err(self.error(format!("Unexpected token {:?}", self.current_type())));
        }

        Ok(self.builder.finish(root))
    }

    fn parse_expression(&mut self, min_bp: u8) -> Result<NodeId, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            if let Some((op, left_bp, right_bp)) = self.infix_binding_power() {
                if left_bp < min_bp {
                    break;
                }

                self.advance();

                let rhs = self.parse_expression(right_bp)?;

                lhs = self.builder.binary(op, lhs, rhs);

                continue;
            }

            if self.can_implicit_multiply() {
                let left_bp = 20;
                let right_bp = 21;

                if left_bp < min_bp {
                    break;
                }

                let rhs = self.parse_expression(right_bp)?;

                lhs = self.builder.binary(BinaryOp::Mul, lhs, rhs);

                continue;
            }

            break;
        }

        Ok(lhs)
    }

    fn can_implicit_multiply(&self) -> bool {
        match self.current_type() {
            Some(TokenType::Number(_)) => true,

            // A function call is also a valid right operand:
            // 3sin(x) => 3 * sin(x)
            Some(TokenType::Identifier(_)) => true,

            Some(TokenType::LeftParen) => true,

            Some(TokenType::AbsoluteValue) => self.absolute_depth == 0,

            _ => false,
        }
    }

    fn parse_prefix(&mut self) -> Result<NodeId, ParseError> {
        let token = self
            .advance()
            .ok_or_else(|| self.error("Unexpected end of expression"))?;

        match token.token_type {
            TokenType::Number(value) => Ok(self.builder.constant(value)),

            TokenType::Identifier(name) => {
                if self.check(&TokenType::LeftParen) && Self::is_known_function(&name) {
                    return self.parse_call(name);
                }

                match name.as_str() {
                    "pi" => Ok(self.builder.constant(std::f64::consts::PI)),

                    "e" => Ok(self.builder.constant(std::f64::consts::E)),

                    _ => Ok(self.builder.variable(name)),
                }
            }

            TokenType::Plus => self.parse_expression(30),

            TokenType::Minus => {
                let input = self.parse_expression(30)?;

                Ok(self.builder.unary(UnaryOp::Neg, input))
            }

            TokenType::LeftParen => {
                let expression = self.parse_expression(0)?;

                self.expect(TokenType::RightParen)?;

                Ok(expression)
            }

            TokenType::AbsoluteValue => {
                self.absolute_depth += 1;

                let result = self.parse_expression(0);

                self.absolute_depth -= 1;

                let input = result?;

                self.expect(TokenType::AbsoluteValue)?;

                Ok(self.builder.unary(UnaryOp::Abs, input))
            }

            _ => Err(ParseError {
                message: format!("Unexpected token {:?}", token.token_type),
                position: token.start,
            }),
        }
    }

    fn is_known_function(name: &str) -> bool {
        matches!(
            name,
            "sin"
                | "cos"
                | "tan"
                | "sqrt"
                | "abs"
                | "ln"
                | "log10"
                | "log"
                | "exp"
                | "pow"
                | "min"
                | "max"
        )
    }

    fn parse_call(&mut self, name: String) -> Result<NodeId, ParseError> {
        self.expect(TokenType::LeftParen)?;

        let mut args = Vec::new();

        if !self.check(&TokenType::RightParen) {
            loop {
                args.push(self.parse_expression(0)?);

                if self.check(&TokenType::Comma) {
                    self.advance();

                    if self.check(&TokenType::RightParen) {
                        return Err(self.error("Missing function argument"));
                    }
                } else {
                    break;
                }
            }
        }

        self.expect(TokenType::RightParen)?;

        self.build_function(&name, args)
    }

    fn build_function(&mut self, name: &str, args: Vec<NodeId>) -> Result<NodeId, ParseError> {
        let unary_op = match name {
            "sin" => Some(UnaryOp::Sin),
            "cos" => Some(UnaryOp::Cos),
            "tan" => Some(UnaryOp::Tan),
            "sqrt" => Some(UnaryOp::Sqrt),
            "abs" => Some(UnaryOp::Abs),
            "ln" => Some(UnaryOp::Ln),
            "log10" => Some(UnaryOp::Log10),
            "exp" => Some(UnaryOp::Exp),
            _ => None,
        };

        if let Some(op) = unary_op {
            if args.len() != 1 {
                return Err(self.error(format!(
                    "Function '{}' expects 1 argument, got {}",
                    name,
                    args.len()
                )));
            }

            return Ok(self.builder.unary(op, args[0]));
        }

        let binary_op = match name {
            "pow" => Some(BinaryOp::Pow),
            "min" => Some(BinaryOp::Min),
            "max" => Some(BinaryOp::Max),
            "log" => Some(BinaryOp::Log),
            _ => None,
        };

        if let Some(op) = binary_op {
            if args.len() != 2 {
                return Err(self.error(format!(
                    "Function '{}' expects 2 arguments, got {}",
                    name,
                    args.len()
                )));
            }

            return Ok(self.builder.binary(op, args[0], args[1]));
        }

        Err(self.error(format!("Unknown function '{}'", name)))
    }

    fn infix_binding_power(&self) -> Option<(BinaryOp, u8, u8)> {
        match self.current_type()? {
            TokenType::Plus => Some((BinaryOp::Add, 10, 11)),

            TokenType::Minus => Some((BinaryOp::Sub, 10, 11)),

            TokenType::Star => Some((BinaryOp::Mul, 20, 21)),

            TokenType::Slash => Some((BinaryOp::Div, 20, 21)),

            TokenType::Percent => Some((BinaryOp::Rem, 20, 21)),

            TokenType::Caret => Some((BinaryOp::Pow, 40, 40)),

            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EvalError {
    UndefinedVariable(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndefinedVariable(name) => {
                write!(f, "Undefined variable '{}'", name)
            }
        }
    }
}

impl std::error::Error for EvalError {}

impl Graph {
    pub fn execute(&self, variables: &HashMap<String, f64>) -> Result<f64, EvalError> {
        let mut values: Vec<f64> = vec![0.0; self.nodes.len()];

        // Evaluate in dependency-first order.
        for id in self.topological_order() {
            let node = &self.nodes[id];

            let value = match &node.kind {
                NodeKind::Constant(value) => *value,

                NodeKind::Variable(name) => *variables
                    .get(name)
                    .ok_or_else(|| EvalError::UndefinedVariable(name.clone()))?,

                NodeKind::Unary { op, input } => {
                    let x = values[*input];

                    match op {
                        UnaryOp::Neg => -x,
                        UnaryOp::Sin => x.sin(),
                        UnaryOp::Cos => x.cos(),
                        UnaryOp::Tan => x.tan(),
                        UnaryOp::Sqrt => x.sqrt(),
                        UnaryOp::Abs => x.abs(),
                        UnaryOp::Ln => x.ln(),
                        UnaryOp::Log10 => x.log10(),
                        UnaryOp::Exp => x.exp(),
                    }
                }

                NodeKind::Binary { op, lhs, rhs } => {
                    let a = values[*lhs];
                    let b = values[*rhs];

                    match op {
                        BinaryOp::Add => a + b,
                        BinaryOp::Sub => a - b,
                        BinaryOp::Mul => a * b,
                        BinaryOp::Div => a / b,
                        BinaryOp::Rem => a % b,
                        BinaryOp::Pow => a.powf(b),
                        BinaryOp::Min => a.min(b),
                        BinaryOp::Max => a.max(b),
                        BinaryOp::Log => a.log(b),
                    }
                }
            };

            values[id] = value;
        }

        Ok(values[self.root])
    }
}
