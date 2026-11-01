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

    pub fn count_nodes(&self) -> usize {
        self.nodes.len()
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

    /// 从 root 做一次可达性分析，剔除不可达节点并重新编号，
    /// 使 `nodes` 紧凑、id 连续。
    ///
    /// 依赖 `GraphBuilder` 始终 append 新节点的性质：
    /// 子节点的旧 id 一定小于父节点，因此按旧 id 递增遍历时，
    /// 父节点可达则其所有子节点也必已可达、`remap` 已填充。
    fn eliminate_dead_nodes(self) -> Graph {
        let mut reachable = vec![false; self.nodes.len()];
        let mut stack = vec![self.root];

        while let Some(id) = stack.pop() {
            if reachable[id] {
                continue;
            }

            reachable[id] = true;

            if let Some(deps) = self.dependencies(id) {
                for dep in deps {
                    if !reachable[dep] {
                        stack.push(dep);
                    }
                }
            }
        }

        let Graph { nodes, root } = self;
        let len = nodes.len();

        let mut remap = vec![usize::MAX; len];
        let mut new_nodes = Vec::with_capacity(len);

        for (old_id, node) in nodes.into_iter().enumerate() {
            if !reachable[old_id] {
                continue;
            }

            let new_id = new_nodes.len();
            remap[old_id] = new_id;

            let kind = match node.kind {
                NodeKind::Constant(value) => NodeKind::Constant(value),

                NodeKind::Variable(name) => NodeKind::Variable(name),

                NodeKind::Unary { op, input } => NodeKind::Unary {
                    op,
                    input: remap[input],
                },

                NodeKind::Binary { op, lhs, rhs } => NodeKind::Binary {
                    op,
                    lhs: remap[lhs],
                    rhs: remap[rhs],
                },
            };

            new_nodes.push(Node { id: new_id, kind });
        }

        Graph {
            nodes: new_nodes,
            root: remap[root],
        }
    }

    /// 单轮重写：按拓扑顺序用 **全新的** `GraphBuilder` 重建整张图。
    /// 由于 builder 的 `unary` / `binary` 会应用全部代数规则、常量折叠和 CSE，
    /// 这一轮就能把上一轮化简后暴露出来的新机会全部吃掉。
    ///
    /// 返回 `(新图, 是否有变化)`。
    fn optimize_round(&self) -> (Graph, bool) {
        let mut builder = GraphBuilder::new();
        let mut remap: Vec<Option<NodeId>> = vec![None; self.nodes.len()];

        // 由于 `eliminate_dead_nodes` 保留了旧 id 的相对顺序，
        // 且原 `GraphBuilder` 的 append 顺序就是拓扑顺序（子先父后），
        // 这里按 id 递增即可保证依赖已 remap。
        for old_id in 0..self.nodes.len() {
            let node = &self.nodes[old_id];

            let new_id = match &node.kind {
                NodeKind::Constant(value) => builder.constant(*value),
                NodeKind::Variable(name) => builder.variable(name.clone()),

                NodeKind::Unary { op, input } => {
                    let input = remap[*input]
                        .expect("topological order violated: unary input not remapped");
                    builder.unary(*op, input)
                }

                NodeKind::Binary { op, lhs, rhs } => {
                    let lhs =
                        remap[*lhs].expect("topological order violated: binary lhs not remapped");
                    let rhs =
                        remap[*rhs].expect("topological order violated: binary rhs not remapped");
                    builder.binary(*op, lhs, rhs)
                }
            };

            remap[old_id] = Some(new_id);
        }

        let new_root = remap[self.root].expect("root must be remapped");

        // 单轮 DCE，不递归 `finish`，避免无限递归。
        let new_graph = builder.finish_round(new_root);

        let changed = !graphs_equivalent(self, &new_graph);
        (new_graph, changed)
    }
}

/// 判断两个图在结构上是否完全等价（节点顺序、种类、引用 id 全部一致）。
fn graphs_equivalent(a: &Graph, b: &Graph) -> bool {
    if a.nodes.len() != b.nodes.len() || a.root != b.root {
        return false;
    }

    a.nodes
        .iter()
        .zip(&b.nodes)
        .all(|(na, nb)| na.id == nb.id && node_kind_eq(&na.kind, &nb.kind))
}

/// 对 `NodeKind` 做结构相等比较；`f64` 用 bit 比较以对 `NaN` 也稳定。
fn node_kind_eq(a: &NodeKind, b: &NodeKind) -> bool {
    match (a, b) {
        (NodeKind::Constant(x), NodeKind::Constant(y)) => x.to_bits() == y.to_bits(),
        (NodeKind::Variable(x), NodeKind::Variable(y)) => x == y,

        (NodeKind::Unary { op: oa, input: ia }, NodeKind::Unary { op: ob, input: ib }) => {
            oa == ob && ia == ib
        }

        (
            NodeKind::Binary {
                op: oa,
                lhs: la,
                rhs: ra,
            },
            NodeKind::Binary {
                op: ob,
                lhs: lb,
                rhs: rb,
            },
        ) => oa == ob && la == lb && ra == rb,

        _ => false,
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

    /// 如果 `id` 是 `Neg(x)`，返回 `Some(x)`。
    fn neg_inner(&self, id: NodeId) -> Option<NodeId> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Unary {
                op: UnaryOp::Neg,
                input,
            } => Some(*input),
            _ => None,
        }
    }

    /// 如果 `id` 是 `Add(x, y)`，返回 `Some((x, y))`。
    fn add_parts(&self, id: NodeId) -> Option<(NodeId, NodeId)> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Binary {
                op: BinaryOp::Add,
                lhs,
                rhs,
            } => Some((*lhs, *rhs)),
            _ => None,
        }
    }

    /// 如果 `id` 是 `Sub(x, y)`，返回 `Some((x, y))`。
    fn sub_parts(&self, id: NodeId) -> Option<(NodeId, NodeId)> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Binary {
                op: BinaryOp::Sub,
                lhs,
                rhs,
            } => Some((*lhs, *rhs)),
            _ => None,
        }
    }

    /// 如果 `id` 是 `Mul(x, y)`，返回 `Some((x, y))`。
    fn mul_parts(&self, id: NodeId) -> Option<(NodeId, NodeId)> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Binary {
                op: BinaryOp::Mul,
                lhs,
                rhs,
            } => Some((*lhs, *rhs)),
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

        // 1) 常量折叠
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

        // 2) 代数恒等式
        match op {
            UnaryOp::Neg => {
                // -(-x) => x
                if let Some(inner) = self.neg_inner(input) {
                    return inner;
                }

                // 新增：-(a - b) => b - a
                if let Some((sub_l, sub_r)) = self.sub_parts(input) {
                    return self.binary(BinaryOp::Sub, sub_r, sub_l);
                }
            }

            UnaryOp::Abs => {
                // ||x|| => |x|
                if let NodeKind::Unary {
                    op: UnaryOp::Abs, ..
                } = &self.nodes[input].kind
                {
                    return input;
                }

                // |-x| => |x|
                if let Some(inner) = self.neg_inner(input) {
                    return self.unary(UnaryOp::Abs, inner);
                }
            }

            _ => {}
        }

        // 3) CSE
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

                // x + (-y) => x - y
                if let Some(inner) = self.neg_inner(rhs) {
                    return self.binary(BinaryOp::Sub, lhs, inner);
                }

                // (-x) + y => y - x
                if let Some(inner) = self.neg_inner(lhs) {
                    return self.binary(BinaryOp::Sub, rhs, inner);
                }

                // x + x => 2 * x
                if lhs == rhs {
                    let two = self.constant(2.0);
                    return self.binary(BinaryOp::Mul, two, lhs);
                }

                // 新增：(a - b) + b => a
                if let Some((sub_l, sub_r)) = self.sub_parts(lhs) {
                    if sub_r == rhs {
                        return sub_l;
                    }
                }

                // b + (a - b) => a
                if let Some((sub_l, sub_r)) = self.sub_parts(rhs) {
                    if sub_r == lhs {
                        return sub_l;
                    }
                }
            }

            BinaryOp::Sub => {
                if rhs_value == Some(0.0) {
                    return lhs;
                }

                // x - (-y) => x + y
                if let Some(inner) = self.neg_inner(rhs) {
                    return self.binary(BinaryOp::Add, lhs, inner);
                }

                // 0 - x => -x
                if lhs_value == Some(0.0) {
                    return self.unary(UnaryOp::Neg, rhs);
                }

                // x - x => 0
                if lhs == rhs {
                    return self.constant(0.0);
                }

                // (a + b) - b => a
                // (a + b) - a => b
                if let Some((add_l, add_r)) = self.add_parts(lhs) {
                    if add_r == rhs {
                        return add_l;
                    }
                    if add_l == rhs {
                        return add_r;
                    }
                }

                // a - (a - b) => b
                if let Some((sub_l, sub_r)) = self.sub_parts(rhs) {
                    if sub_l == lhs {
                        return sub_r;
                    }
                }
            }

            BinaryOp::Mul => {
                if rhs_value == Some(1.0) {
                    return lhs;
                }

                if lhs_value == Some(1.0) {
                    return rhs;
                }

                // x * 0 => 0,  0 * x => 0
                if rhs_value == Some(0.0) || lhs_value == Some(0.0) {
                    return self.constant(0.0);
                }

                // x * (-1) => -x,  (-1) * x => -x
                if rhs_value == Some(-1.0) {
                    return self.unary(UnaryOp::Neg, lhs);
                }

                if lhs_value == Some(-1.0) {
                    return self.unary(UnaryOp::Neg, rhs);
                }

                // (-x) * (-y) => x * y
                if let (Some(l_inner), Some(r_inner)) = (self.neg_inner(lhs), self.neg_inner(rhs)) {
                    return self.binary(BinaryOp::Mul, l_inner, r_inner);
                }

                // (-x) * y => -(x * y)
                if let Some(l_inner) = self.neg_inner(lhs) {
                    let prod = self.binary(BinaryOp::Mul, l_inner, rhs);
                    return self.unary(UnaryOp::Neg, prod);
                }

                // x * (-y) => -(x * y)
                if let Some(r_inner) = self.neg_inner(rhs) {
                    let prod = self.binary(BinaryOp::Mul, lhs, r_inner);
                    return self.unary(UnaryOp::Neg, prod);
                }
            }

            BinaryOp::Div => {
                if rhs_value == Some(1.0) {
                    return lhs;
                }

                // (-x) / (-y) => x / y
                if let (Some(l_inner), Some(r_inner)) = (self.neg_inner(lhs), self.neg_inner(rhs)) {
                    return self.binary(BinaryOp::Div, l_inner, r_inner);
                }

                // (-x) / y => -(x / y)
                if let Some(l_inner) = self.neg_inner(lhs) {
                    let q = self.binary(BinaryOp::Div, l_inner, rhs);
                    return self.unary(UnaryOp::Neg, q);
                }

                // x / (-y) => -(x / y)
                if let Some(r_inner) = self.neg_inner(rhs) {
                    let q = self.binary(BinaryOp::Div, lhs, r_inner);
                    return self.unary(UnaryOp::Neg, q);
                }

                // (a * k) / k => a
                if let Some(k) = rhs_value {
                    if k != 0.0 && k.is_finite() {
                        if let Some((mul_l, mul_r)) = self.mul_parts(lhs) {
                            if self.constant_value(mul_l) == Some(k) {
                                return mul_r;
                            }
                            if self.constant_value(mul_r) == Some(k) {
                                return mul_l;
                            }
                        }
                    }
                }
            }

            BinaryOp::Pow => {
                if rhs_value == Some(1.0) {
                    return lhs;
                }

                // x^0 => 1
                if rhs_value == Some(0.0) {
                    return self.constant(1.0);
                }

                // 1^x => 1
                if lhs_value == Some(1.0) {
                    return self.constant(1.0);
                }

                // x^2 => x * x
                if rhs_value == Some(2.0) {
                    return self.binary(BinaryOp::Mul, lhs, lhs);
                }

                // x^0.5 => sqrt(x)
                if rhs_value == Some(0.5) {
                    return self.unary(UnaryOp::Sqrt, lhs);
                }

                // x^3 => (x * x) * x
                if rhs_value == Some(3.0) {
                    let sq = self.binary(BinaryOp::Mul, lhs, lhs);
                    return self.binary(BinaryOp::Mul, sq, lhs);
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

    fn finish_round(self, root: NodeId) -> Graph {
        Graph {
            nodes: self.nodes,
            root,
        }
        .eliminate_dead_nodes()
    }

    pub fn finish(self, root: NodeId) -> Graph {
        assert!(root < self.nodes.len());

        let mut graph = self.finish_round(root);

        const MAX_ROUNDS: usize = 32;

        for _ in 0..MAX_ROUNDS {
            let (new_graph, changed) = graph.optimize_round();
            if !changed {
                break;
            }
            graph = new_graph;
        }

        graph
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
