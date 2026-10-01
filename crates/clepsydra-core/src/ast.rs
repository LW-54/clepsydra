use crate::arena::{Arena, ArenaId};

pub type ExprId<'brand, V> = ArenaId<'brand, Expr<'brand, V>>;

#[derive(Debug, PartialEq, Eq)]
pub enum Expr<'brand, V> {
    Var(V),
    Const(u64),
    Add(ExprId<'brand, V>, ExprId<'brand, V>),
    SatSub(ExprId<'brand, V>, ExprId<'brand, V>),
}

pub type AST<'brand, V> = Arena<'brand, Expr<'brand, V>>;

pub trait AstBuilder<'brand, V> {
    fn const_val(&mut self, value: u64) -> ExprId<'brand, V>;
    fn var(&mut self, variable: V) -> ExprId<'brand, V>;
    fn add(&mut self, lhs: ExprId<'brand, V>, rhs: ExprId<'brand, V>) -> ExprId<'brand, V>;
    fn sat_sub(&mut self, lhs: ExprId<'brand, V>, rhs: ExprId<'brand, V>) -> ExprId<'brand, V>;
}

impl<'brand, V: PartialEq> AstBuilder<'brand, V> for AST<'brand, V> {
    fn const_val(&mut self, value: u64) -> ExprId<'brand, V> {
        self.push(Expr::Const(value))
    }

    fn var(&mut self, variable: V) -> ExprId<'brand, V> {
        self.push(Expr::Var(variable))
    }

    fn add(&mut self, lhs: ExprId<'brand, V>, rhs: ExprId<'brand, V>) -> ExprId<'brand, V> {
        match (self.get(lhs), self.get(rhs)) {
            (Expr::Const(0), _) => rhs,
            (_, Expr::Const(0)) => lhs,
            (Expr::Const(a), Expr::Const(b)) => self.const_val(a.saturating_add(*b)),
            _ => self.push(Expr::Add(lhs, rhs)),
        }
    }

    fn sat_sub(&mut self, lhs: ExprId<'brand, V>, rhs: ExprId<'brand, V>) -> ExprId<'brand, V> {
        match (self.get(lhs), self.get(rhs)) {
            (a, b) if a == b => self.const_val(0),
            (_, Expr::Const(0)) => lhs,
            (Expr::Const(a), Expr::Const(b)) => self.const_val(a.saturating_sub(*b)),
            _ => self.push(Expr::SatSub(lhs, rhs)),
        }
    }
}
