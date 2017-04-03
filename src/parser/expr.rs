//! trashcan's sub-parsers for expressions

use std::str;

use super::{ParseError, ParseResult, SrcLoc};
#[macro_use]
use super::bits::*;
use super::op::*;
use super::lit::*;
use super::ident::*;

use ast::*;

// we have to handle left recursion very carefully
//   when parsing expressions (see https://en.wikipedia.org/wiki/Left_recursion)
//   and an example at https://github.com/Geal/nom/blob/master/tests/arithmetic_ast.rs

// the "rest" (recursive part) of a recursive expr
#[derive(Debug)]
enum RecExprRest {
    CondExpr(Expr, Expr, usize),
}

#[derive(Debug)]
// the "rest" (recursive part) of a "unitary" recursive expr
enum UnitaryRecExprRest {
    Indexed(Vec<Expr>, usize),
    Member(Ident, usize),
    MemberInvoke(Ident, Vec<Expr>, usize),
    Cast(Type, usize),
    // FunCall(Vec<Expr>),
}

// official table of intended operator precedence!
//   shamelessly stolen from C
// 1 : arrays[], fncalls(), (parens), weird(mbr).invokes
// 2 : !unaryops
// 3 : ^
// 4 : * / %
// 5 : + - @
// 6 : > < >= <=
// 7 : == !=
// 8 : &
// 9 : |
// 10: &&
// 11: ||
// 12: x ? y : z

// pull a nonrecursive expr, and maybe a recursive rest
#[inline]
pub fn expr(input: &[u8]) -> ParseResult<Expr> {
    // let (i, first) = require!(logorexpr(input));
    let (i, first) = require!(powexpr(input));
    let (i, rest) = require!(opt(i, condexpr));
    let e = match rest {
        None => first,
        Some(RecExprRest::CondExpr(ifexpr, elseexpr, len)) => {
           let loc = SrcLoc::raw(first.loc.start, first.loc.len + len);
           Expr {
               data: ExprKind::CondExpr {
                         cond: Box::new(first),
                         if_expr: Box::new(ifexpr),
                         else_expr: Box::new(elseexpr),
                     },
               loc,
           }
       },
    };
    ok!(i, e)
}

fn fold_bin_exprs(first: Expr, rest: Vec<(BinOp, Expr)>) -> Expr {
    rest.into_iter().fold(first, |sofar, (op, e)| {
        let loc = SrcLoc::raw(
            sofar.loc.start,
            sofar.loc.len +
              (e.loc.start - sofar.loc.len - sofar.loc.start) +
              e.loc.len
        );

        Expr {
            data: ExprKind::BinOpApp(Box::new(sofar), Box::new(e), op),
            loc,
        }
    })
}

// non-unitary exprs, in decreasing precedence order...

pub fn powexpr(input: &[u8]) -> ParseResult<Expr> {
    let (i, first) = require!(unitary_op_expr(input));
    let (i, rest) = require!(many(i, |i| {
        let (i, op) = require!(pow_op(i));
        let (i, e) = require!(unitary_op_expr(i));
        ok!(i, (op, e))
    }));
    ok!(i, fold_bin_exprs(first, rest))
}

/*

named!(muldivexpr<Expr>, complete!(do_parse!(
    first: powexpr >>
     rest: many0!(tuple!(muldiv_op, powexpr)) >>
           (fold_bin_exprs(first, rest))
)));

named!(addsubexpr<Expr>, complete!(do_parse!(
    first: muldivexpr >>
     rest: many0!(tuple!(addsub_op, muldivexpr)) >>
           (fold_bin_exprs(first, rest))
)));

named!(cmpexpr<Expr>, complete!(do_parse!(
    first: addsubexpr >>
     rest: many0!(tuple!(cmp_op, addsubexpr)) >>
           (fold_bin_exprs(first, rest))
)));

named!(eqexpr<Expr>, complete!(do_parse!(
    first: cmpexpr >>
     rest: many0!(tuple!(eq_op, cmpexpr)) >>
           (fold_bin_exprs(first, rest))
)));

named!(bitandexpr<Expr>, complete!(do_parse!(
    first: eqexpr >>
     rest: many0!(tuple!(bitand_op, eqexpr)) >>
           (fold_bin_exprs(first, rest))
)));

named!(bitorexpr<Expr>, complete!(do_parse!(
    first: bitandexpr >>
     rest: many0!(tuple!(bitor_op, bitandexpr)) >>
           (fold_bin_exprs(first, rest))
)));

named!(logandexpr<Expr>, complete!(do_parse!(
    first: bitorexpr >>
     rest: many0!(tuple!(logand_op, bitorexpr)) >>
           (fold_bin_exprs(first, rest))
)));

named!(logorexpr<Expr>, complete!(do_parse!(
    first: logandexpr >>
     rest: many0!(tuple!(logor_op, logandexpr)) >>
           (fold_bin_exprs(first, rest))
)));

*/

// the rest (? xxx : yyy) of a conditional expr
fn condexpr(input: &[u8]) -> ParseResult<RecExprRest> {
    let (i, start_pos) = require!(pos(input));
    let (i, _) = opt(i, multispace)?;
    let (i, _) = require!(byte(i, b'?'));

    // cut on error after ?
    let (i, ifexpr) = require_or_cut!(expr(i) => ParseError::ExpectedExpr);

    let (i, _) = opt(i, multispace)?;
    let (i, _) = require_or_cut!(byte(i, b':'));

    let (i, elseexpr) = require_or_cut!(expr(i) => ParseError::ExpectedExpr);

    let (i, end_pos) = require!(pos(i));
    ok!(i, RecExprRest::CondExpr(ifexpr, elseexpr, end_pos - start_pos))
}

// "unitary" exprs, possibly preceded by unary operators
#[inline]
fn unitary_op_expr(input: &[u8]) -> ParseResult<Expr> {
    alt!(input,
        unitary_expr(input)
      ; unitary_op_expr1(input)
    )
}

// "unitary" expr with exactly one unary op applied
#[inline]
fn unitary_op_expr1(input: &[u8]) -> ParseResult<Expr> {
    let (i, _) = opt(input, multispace)?;
    let (i, start_pos) = require!(pos(i));
    let (i, op) = require!(un_op(i));
    // TODO: maybe cut after here?
    let (i, e) = require!(unitary_op_expr(i));
    let (i, end_pos) = require!(pos(i));
    ok!(i, Expr {
        data: ExprKind::UnOpApp(Box::new(e), op),
        loc: SrcLoc::raw(start_pos, end_pos - start_pos),
    })
}

// "unitary" exprs (bind to unary ops for precedence)
// pull a nonrecursive expr, and maybe a recursive rest
#[inline]
fn unitary_expr(input: &[u8]) -> ParseResult<Expr> {
    let (i, first) = require!(nonrec_unitary_expr(input));
    let (i, rest) = require!(many(i, |i| alt!(i,
        indexed(i)
      ; memberinvoke(i)
      ; member(i)
      ; cast(i)
    )));
    ok!(i, fold_unitary_exprs(first, rest))
}

// a non left-recursive unitary expr
#[inline]
fn nonrec_unitary_expr(input: &[u8]) -> ParseResult<Expr> {
    alt!(input,
   //   fncall
        extent_expr(input)
      ; pathexpr(input)
      ; litexpr(input)
      ; grouped(input)
      ; vbexpr(input)
    )
}

fn fold_unitary_exprs(first: Expr, rest: Vec<UnitaryRecExprRest>) -> Expr {
    rest.into_iter().fold(first, |sofar, rest| {
        match rest {
            UnitaryRecExprRest::Indexed(indices, len) => {
                let loc = SrcLoc::raw(sofar.loc.start, sofar.loc.len + len);
                Expr {
                    data: ExprKind::Index(Box::new(sofar), indices),
                    loc,
                }
            },

            UnitaryRecExprRest::Member(i, len) => {
                let loc = SrcLoc::raw(sofar.loc.start, sofar.loc.len + len);
                Expr {
                    data: ExprKind::Member(Box::new(sofar), i),
                    loc,
                }
            },

            UnitaryRecExprRest::MemberInvoke(i, args, len) => {
                let loc = SrcLoc::raw(sofar.loc.start, sofar.loc.len + len);
                Expr {
                    data: ExprKind::MemberInvoke(Box::new(sofar), i, args),
                    loc,
                }
            },

            UnitaryRecExprRest::Cast(ty, len) => {
                let loc = SrcLoc::raw(sofar.loc.start, sofar.loc.len + len);
                Expr {
                    data: ExprKind::Cast(Box::new(sofar), ty),
                    loc,
                }
            },
        }
    })
}

/*

named!(pub fncall<Expr>, complete!(do_parse!(
            opt!(call!(nom::multispace)) >>
 start_pos: call!(super::pos) >>
      name: call!(path) >>
            opt!(call!(nom::multispace)) >>
            char!('(') >>
      args: separated_list!(ws!(char!(',')), expr) >>
            opt!(call!(nom::multispace)) >>
   optargs: opt!(preceded!(
                char!(';'),
                separated_nonempty_list!(ws!(char!(',')), optarg)
            )) >>
            opt!(call!(nom::multispace)) >>
            char!(')') >>
   end_pos: call!(super::pos) >>
            (Expr {
                data: ExprKind::Call(name, args, optargs.unwrap_or(Vec::new())),
                loc: SrcLoc::raw(start_pos, end_pos - start_pos),
            })
)));

named!(pub optarg<(Ident, Expr)>, dbg_dmp!(complete!(do_parse!(
        opt!(call!(nom::multispace)) >>
  name: ident >>
        opt!(call!(nom::multispace)) >>
        char!('=') >>
        opt!(call!(nom::multispace)) >>
   arg: expr >>
        (name, arg)
))));

*/

// a path as an expression
fn pathexpr(input: &[u8]) -> ParseResult<Expr> {
    let (i, _) = opt(input, multispace)?;
    let (i, start_pos) = require!(pos(i));
    let (i, p) = require!(path(i));
    let (i, end_pos) = require!(pos(i));
    ok!(i, Expr {
        data: ExprKind::Name(p),
        loc: SrcLoc::raw(start_pos, end_pos - start_pos),
    })
}

// a literal as an expression
fn litexpr(input: &[u8]) -> ParseResult<Expr> {
    let (i, _) = opt(input, multispace)?;
    let (i, start_pos) = require!(pos(i));
    let (i, lit) = require!(literal(i));
    let (i, end_pos) = require!(pos(i));
    ok!(i, Expr {
        data: ExprKind::Lit(lit),
        loc: SrcLoc::raw(start_pos, end_pos - start_pos),
    })
}

// an expr grouped in parentheses, to force precedence
fn grouped(input: &[u8]) -> ParseResult<Expr> {
    let (i, _) = opt(input, multispace)?;
    let (i, start_pos) = require!(pos(i));
    let (i, _) = require!(byte(i, b'('));
    let (i, e) = require!(expr(i));
    // TODO: should we cut on fail after here?
    let (i, _) = opt(input, multispace)?;
    let (i, _) = require!(byte(i, b')'));
    let (i, end_pos) = require!(pos(i));
    ok!(i, Expr {
        data: e.data,
        loc: SrcLoc::raw(start_pos, end_pos - start_pos),
    })
}

// an extents expression e.g. first_index<0>(arr)
fn extent_expr(input: &[u8]) -> ParseResult<Expr> {
    let (i, _) = opt(input, multispace)?;
    let (i, start_pos) = require!(pos(i));

    let (i, kind) = require!(alt!(i,
        keyword_immediate(i, b"first_index") => |_| ExtentKind::First
      ; keyword_immediate(i, b"last_index") => |_| ExtentKind::Last
      ; keyword_immediate(i, b"array_length") => |_| ExtentKind::Length
    ));

    // cut on error after this point
    let (i, _) = opt(i, multispace)?;
    let (i, _) = require_or_cut!(byte(i, b'<'));
    let (i, _) = opt(i, multispace)?;

    let (i_after_dim, dim) = require_or_cut!(digits(i)
      => ParseError::ExpectedDimSpecifier);

    let dim = match unsafe { str::from_utf8_unchecked(dim) }.parse::<usize>() {
        Ok(dim) => dim,
        Err(_) => return cut!(i, ParseError::InvalidLiteral),
    };

    let (i, _) = opt(i_after_dim, multispace)?;
    let (i, _) = require_or_cut!(byte(i, b'>'));
    let (i, _) = opt(i_after_dim, multispace)?;
    let (i, _) = require_or_cut!(byte(i, b'('));

    let (i, arr) = require_or_cut!(expr(i) => ParseError::ExpectedExpr);

    let (i, _) = opt(i_after_dim, multispace)?;
    let (i, _) = require_or_cut!(byte(i, b')'));

    let (i, end_pos) = require!(pos(i));

    ok!(i, Expr {
        data: ExprKind::ExtentExpr(Box::new(arr), kind, dim),
        loc: SrcLoc::raw(start_pos, end_pos - start_pos),
    })
}

// a passthrough VB expression
// TODO: this needs work to handle escaping ` inside vb stmts
fn vbexpr(input: &[u8]) -> ParseResult<Expr> {
    let (i, _) = opt(input, multispace)?;
    let (i, start_pos) = require!(pos(i));
    let (i, _) = require!(byte(i, b'`'));
    let (i, vb) = require!(bytes_not(i, b'`'));
    let (i, _) = require!(byte(i, b'`'));
    let (i, end_pos) = require!(pos(i));
    ok!(i, Expr {
        data: ExprKind::VbExpr(Vec::from(vb)),
        loc: SrcLoc::raw(start_pos, end_pos - start_pos),
    })
}

// various possible recursive "rests" of unitary exprs

fn indexed(input: &[u8]) -> ParseResult<UnitaryRecExprRest> {
    let (i, start_pos) = require!(pos(input));
    let (i, _) = opt(i, multispace)?;
    let (i, _) = require!(byte(i, b'['));
    // cut on error after this point
    let (i, indices) = require_or_cut!(delimited_at_least_one(i,
        expr,
        |i| chain!(i,
            |i| opt(i, multispace) =>
            |i| byte(i, b',')
        )) => ParseError::ExpectedExpr);
    let (i, _) = opt(i, multispace)?;
    let (i, _) = require_or_cut!(byte(i, b']'));
    let (i, end_pos) = require_or_cut!(pos(i));
    ok!(i, UnitaryRecExprRest::Indexed(indices, end_pos - start_pos))
}

fn member(input: &[u8]) -> ParseResult<UnitaryRecExprRest> {
    let (i, start_pos) = require!(pos(input));
    let (i, _) = opt(i, multispace)?;
    let (i, _) = require!(byte(i, b'.'));
    // at this point, we need to cut on error (we run after memberinvoke, so
    //   we need to cut if no ident found after dot)
    let (i, name) = require_or_cut!(ident(i) => ParseError::ExpectedIdent);
    let (i, end_pos) = require_or_cut!(pos(i));
    ok!(i, UnitaryRecExprRest::Member(name, end_pos - start_pos))
}

fn memberinvoke(input: &[u8]) -> ParseResult<UnitaryRecExprRest> {
    let (i, start_pos) = require!(pos(input));
    let (i, _) = opt(i, multispace)?;
    let (i, _) = require!(byte(i, b'.'));
    let (i, name) = require!(ident(i));
    let (i, _) = require!(byte(i, b'('));
    // at this point, we need to cut on error
    let (i, args) = require_or_cut!(delimited(i,
        expr,
        |i| chain!(i,
            |i| opt(i, multispace) =>
            |i| byte(i, b',')
        )));
    let (i, _) = opt(i, multispace)?;
    let (i, _) = require_or_cut!(byte(i, b')'));
    let (i, end_pos) = require_or_cut!(pos(i));
    ok!(i, UnitaryRecExprRest::MemberInvoke(name, args, end_pos - start_pos))
}

fn cast(input: &[u8]) -> ParseResult<UnitaryRecExprRest> {
    let (i, start_pos) = require!(pos(input));
    let (i, _) = require!(multispace(i));
    let (i, _) = require!(keyword_immediate(i, b"as"));
    // at this point, we need to cut on error
    let (i, _) = require_or_cut!(multispace(i) => ParseError::ExpectedTypename);
    let (i, ty) = require_or_cut!(typename(i) => ParseError::ExpectedTypename);
    let (i, end_pos) = require_or_cut!(pos(i));
    ok!(i, UnitaryRecExprRest::Cast(ty, end_pos - start_pos))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_casts() {
        expect_parse!(cast(b" as i32") =>
          UnitaryRecExprRest::Cast(Type::Int32, 7usize));
        expect_parse!(cast(b" as i32[,,]") =>
          UnitaryRecExprRest::Cast(Type::Array(_, _), 11usize));
        expect_parse_err!(cast(b"(17)") => _);
        expect_parse_cut!(cast(b" as <<<>>>") =>
          ParseError::ExpectedTypename);
    }

    #[test]
    fn parse_members() {
        expect_parse!(member(b" . y") =>
          UnitaryRecExprRest::Member(Ident(_, None), 4));
        expect_parse_cut!(member(b".777") => ParseError::ExpectedIdent);
    }

    #[test]
    fn parse_exprs() {
        expect_parse!(expr(b" `some vb expression`") => Expr {
            data: ExprKind::VbExpr(_),
            ..
        });

        expect_parse!(expr(b"1345.67") => Expr {
            data: ExprKind::Lit(Literal::Float64(1345.67)),
            ..
        });

        expect_parse!(expr(b" some ::\t thing") => Expr {
            data: ExprKind::Name(Path(Some(Ident(_, None)), Ident(_, None))),
            ..
        });
    }
}
