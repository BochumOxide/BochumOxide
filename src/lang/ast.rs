use crate::utils::State;
use anyhow::{bail, Context, Result};
use byteorder::{ByteOrder, LittleEndian, NativeEndian};
use pest::{
    self,
    iterators::{Pair, Pairs},
    Parser,
};
use pest_derive::*;
use std::fmt::Debug;

use super::*;

#[derive(Parser)]
#[grammar = "lang/grammar.pest"]
pub struct GrammarParser;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Operator {
    Add,
    And,
    Div,
    Mod,
    Mul,
    Neg,
    Or,
    Sll,
    Slr,
    Sub,
    Xor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Reg(String),
    Int(i64),
    UnaryExpression {
        operator: Operator,
        child: Box<Node>,
    },
    BinaryExpr {
        // i.e. AddExpr, MulExpr, BitExpr
        operator: Operator,
        lhs: Box<Node>,
        rhs: Box<Node>,
    },
}

pub enum NodeResult {
    Int(i64),
    Bytes(Vec<u8>),
}

impl NodeResult {
    pub fn as_int(self) -> Result<i64> {
        match self {
            NodeResult::Int(i) => Ok(i),
            NodeResult::Bytes(b) => String::from_utf8(b.to_vec())
                .context("Invalid utf8")?
                .parse()
                .context("Invalid number"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ast {
    root: Box<Node>,
}

impl Ast {
    pub fn new(script: &str) -> Result<Self> {
        // parse script and build ast
        let pairs = GrammarParser::parse(Rule::Script, &script)?;
        let root = Box::new(Ast::build_from_expr(pairs.peek().unwrap().into_inner())?);

        Ok(Ast { root })
    }

    pub fn get_result(&self, state: &State) -> Result<Vec<u8>> {
        let res = Ast::evaluate(&self.root, state)?;
        match res {
            NodeResult::Int(i) => Ok(format!("{}", i).into_bytes()),
            NodeResult::Bytes(b) => Ok(b),
        }
    }

    fn evaluate(node: &Node, state: &State) -> Result<NodeResult> {
        match node {
            Node::Int(x) => Ok(NodeResult::Int(*x)),
            Node::Reg(x) => {
                let val = state.registers.get(&x[1..]).context("Invalid Register")?;
                Ok(NodeResult::Bytes(val.to_vec()))
            }
            Node::UnaryExpression { operator, child } => {
                let child = Ast::evaluate(child, state)?.as_int()?;
                Ok(NodeResult::Int(match operator {
                    Operator::Add => child,
                    Operator::Sub => -child,
                    Operator::Neg => !child,
                    _ => unreachable!(),
                }))
            }
            Node::BinaryExpr { operator, lhs, rhs } => {
                let lhs = Ast::evaluate(lhs, state)?.as_int()?;
                let rhs = Ast::evaluate(rhs, state)?.as_int()?;
                Ok(NodeResult::Int(match operator {
                    Operator::Add => lhs + rhs,
                    Operator::And => lhs & rhs,
                    Operator::Div => lhs / rhs,
                    Operator::Mod => lhs % rhs,
                    Operator::Mul => lhs * rhs,
                    Operator::Or => lhs | rhs,
                    Operator::Sll => lhs << rhs,
                    Operator::Slr => lhs >> rhs,
                    Operator::Sub => lhs - rhs,
                    Operator::Xor => lhs ^ rhs,
                    Operator::Neg => panic!("Negation is not a binary operator"),
                }))
            }
        }
    }

    fn build_from_expr(pairs: Pairs<Rule>) -> Result<Node> {
        match pairs.peek().unwrap().as_rule() {
            Rule::AddExpr | Rule::MulExpr | Rule::BitExpr => {
                let mut pairs_iter = pairs.clone().into_iter();

                let lhs_pair = pairs_iter.next();
                let op_pair = pairs_iter.next();
                let rhs_pair = pairs_iter.next();

                // if lhs, rhs and operator exist -> build node, continue with inner
                if lhs_pair.is_some() && op_pair.is_some() && rhs_pair.is_some() {
                    // unpack expressions
                    let lhs = Ast::build_from_expr(lhs_pair.unwrap().into_inner())?;
                    let rhs = Ast::build_from_expr(rhs_pair.unwrap().into_inner())?;

                    let operator = op_pair.unwrap();

                    Ok(Ast::build_from_binary_expr(operator, lhs, rhs))
                } else {
                    // otherwise, unpack expression
                    Ast::build_from_expr(pairs.peek().unwrap().into_inner())
                }
            }
            Rule::UnaryExpr => {
                let mut pairs_iter = pairs.peek().unwrap().into_inner();

                let op_pair = pairs_iter.next();
                let child_pair = pairs_iter.next();

                // if unary expr packs operator and unary expr
                if op_pair.is_some() && child_pair.is_some() {
                    let child = Ast::build_from_term(child_pair.unwrap().into_inner())?;
                    Ok(Ast::build_from_unary_expr(op_pair.unwrap(), child))
                } else {
                    // otherwise, unary expr packs terminal
                    Ast::build_from_term(pairs.peek().unwrap().into_inner())
                }
            }
            Rule::Term => Ast::build_from_term(pairs),
            unknown => bail!(format!("unknown rule {:?}", unknown)),
        }
    }

    fn build_from_unary_expr(operator: Pair<Rule>, child: Node) -> Node {
        Node::UnaryExpression {
            operator: match operator.as_str() {
                "+" => Operator::Add,
                "-" => Operator::Sub,
                "~" => Operator::Neg,
                _ => unreachable!(),
            },
            child: Box::new(child),
        }
    }

    fn build_from_binary_expr(operator: Pair<Rule>, lhs: Node, rhs: Node) -> Node {
        Node::BinaryExpr {
            operator: match operator.as_str() {
                "%" => Operator::Mod,
                "&" => Operator::And,
                "*" => Operator::Mul,
                "+" => Operator::Add,
                "-" => Operator::Sub,
                "/" => Operator::Div,
                "<<" => Operator::Sll,
                ">>" => Operator::Slr,
                "^" => Operator::Xor,
                "|" => Operator::Or,
                _ => unreachable!(),
            },
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn build_from_term(pairs: Pairs<Rule>) -> Result<Node> {
        let rule = pairs.peek().unwrap().as_rule();
        let pair = pairs.peek().unwrap();
        match rule {
            Rule::HexInt => {
                let hex_int_str = &pair.as_str()[2..];
                let hex_int = i64::from_str_radix(&hex_int_str, 16)
                    .context("Hex string to int conversion failed.")?;
                Ok(Node::Int(hex_int))
            }
            Rule::DecInt => {
                let int_str = pair.as_str();
                let int: i64 = int_str.parse().context("Could not parse string to int")?;
                Ok(Node::Int(int))
            }
            Rule::Register => Ok(Node::Reg(pair.as_str().to_owned())),
            Rule::AddExpr | Rule::MulExpr | Rule::BitExpr => Ast::build_from_expr(pairs),
            unknown => bail!("Unknown term: {:?}", unknown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::Target;

    use pest::{consumes_to, parses_to};

    #[test]
    fn parse_unary_expr_no_operator() {
        parses_to! {
            parser: GrammarParser,
            input: "1337",
            rule: Rule::UnaryExpr,
            tokens: [
                UnaryExpr(0, 4, [
                    DecInt(0, 4)
                ]),
            ]
        };
    }

    #[test]
    fn parse_unary_expr_operator() {
        parses_to! {
            parser: GrammarParser,
            input: "-1337",
            rule: Rule::UnaryExpr,
            tokens: [
                UnaryExpr(0, 5, [
                    UnaryOperator(0, 1),
                    UnaryExpr(1, 5, [
                        DecInt(1, 5),
                    ]),
                ]),
            ]
        };
    }

    #[test]
    fn parse_binary_expr() {
        parses_to! {
            parser: GrammarParser,
            input: "1337 + 0x4242",
            rule: Rule::AddExpr,
            tokens: [
                AddExpr(0, 13, [
                    MulExpr(0, 4, [
                        BitExpr(0, 4, [
                            UnaryExpr(0, 4, [
                                DecInt(0, 4),
                            ]),
                        ]),
                    ]),
                    AddOperator(5, 6),
                    AddExpr(7, 13, [
                        MulExpr(7, 13, [
                            BitExpr(7, 13, [
                                UnaryExpr(7, 13, [
                                    HexInt(7, 13),
                                ]),
                            ]),
                        ]),
                    ]),
                ]),
            ]
        }
    }

    #[test]
    fn ast_script() {
        let ast = Ast::new("1337 + 0x4242 * 375");
        assert!(ast.is_ok());

        assert_eq!(
            ast.unwrap(),
            Ast {
                root: Box::new(Node::BinaryExpr {
                    operator: Operator::Add,
                    lhs: Box::new(Node::Int(1337)),
                    rhs: Box::new(Node::BinaryExpr {
                        operator: Operator::Mul,
                        lhs: Box::new(Node::Int(0x4242)),
                        rhs: Box::new(Node::Int(375)),
                    }),
                }),
            }
        )
    }

    #[test]
    fn ast_evaluate() {
        let state = State::new(Target::Local, "cat", &[]).unwrap();
        let ast = Ast::new("1 + 2 * 3 - 4 * 0x1");
        assert!(ast.is_ok());

        let result = ast.unwrap().get_result(&state);
        assert!(result.is_ok());

        assert_eq!(result.unwrap(), [51]);
    }
}
