//! Function plots: `y = f(x)` curves with axes, ticks and labels, made of
//! ordinary objects (path shapes and texts in one group), so they can be
//! edited, restyled and exported like the rest of a drawing.
//!
//! Functions are written in plain notation (`sin(x)^2 + exp(-x^2/2)`) or
//! LaTeX (`\sin^2 x + e^{-x^2/2}`, `\frac{1}{1+x^2}`, `\sqrt{x}`, `|x|`).

use crate::kurbo::Point;
use crate::model::{Anchor, Document, Group, PlotSource, Shape, ShapeKind, Text};

// ---- expressions -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Sec,
    Csc,
    Cot,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Exp,
    Ln,
    Log10,
    Log2,
    Sqrt,
    Abs,
    Sign,
    Floor,
    Ceil,
}

const FUNCS: &[(&str, Func)] = &[
    ("arcsin", Func::Asin),
    ("arccos", Func::Acos),
    ("arctan", Func::Atan),
    ("asin", Func::Asin),
    ("acos", Func::Acos),
    ("atan", Func::Atan),
    ("sinh", Func::Sinh),
    ("cosh", Func::Cosh),
    ("tanh", Func::Tanh),
    ("sin", Func::Sin),
    ("cos", Func::Cos),
    ("tan", Func::Tan),
    ("sec", Func::Sec),
    ("csc", Func::Csc),
    ("cot", Func::Cot),
    ("exp", Func::Exp),
    ("ln", Func::Ln),
    ("log10", Func::Log10),
    ("log2", Func::Log2),
    ("log", Func::Ln),
    ("sqrt", Func::Sqrt),
    ("abs", Func::Abs),
    ("sign", Func::Sign),
    ("sgn", Func::Sign),
    ("floor", Func::Floor),
    ("ceil", Func::Ceil),
];

impl Func {
    fn apply(self, v: f64) -> f64 {
        match self {
            Func::Sin => v.sin(),
            Func::Cos => v.cos(),
            Func::Tan => v.tan(),
            Func::Sec => 1.0 / v.cos(),
            Func::Csc => 1.0 / v.sin(),
            Func::Cot => 1.0 / v.tan(),
            Func::Asin => v.asin(),
            Func::Acos => v.acos(),
            Func::Atan => v.atan(),
            Func::Sinh => v.sinh(),
            Func::Cosh => v.cosh(),
            Func::Tanh => v.tanh(),
            Func::Exp => v.exp(),
            Func::Ln => v.ln(),
            Func::Log10 => v.log10(),
            Func::Log2 => v.log2(),
            Func::Sqrt => v.sqrt(),
            Func::Abs => v.abs(),
            Func::Sign => {
                if v == 0.0 {
                    0.0
                } else {
                    v.signum()
                }
            }
            Func::Floor => v.floor(),
            Func::Ceil => v.ceil(),
        }
    }

    fn name(self) -> &'static str {
        FUNCS
            .iter()
            .find(|(_, f)| *f == self)
            .map_or("f", |(n, _)| n)
    }

    /// LaTeX for the function applied to `arg`.
    fn latex(self, arg: &str) -> String {
        match self {
            Func::Sqrt => format!("\\sqrt{{{arg}}}"),
            Func::Abs => format!("\\left|{arg}\\right|"),
            Func::Exp => format!("e^{{{arg}}}"),
            Func::Floor => format!("\\lfloor {arg} \\rfloor"),
            Func::Ceil => format!("\\lceil {arg} \\rceil"),
            Func::Log10 => format!("\\log_{{10}}\\left({arg}\\right)"),
            Func::Log2 => format!("\\log_{{2}}\\left({arg}\\right)"),
            Func::Asin => format!("\\arcsin\\left({arg}\\right)"),
            Func::Acos => format!("\\arccos\\left({arg}\\right)"),
            Func::Atan => format!("\\arctan\\left({arg}\\right)"),
            Func::Sign => format!("\\operatorname{{sgn}}\\left({arg}\\right)"),
            f => format!("\\{}\\left({arg}\\right)", f.name()),
        }
    }
}

/// A parsed function of `x`.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    X,
    Pi,
    E,
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Root(Box<Expr>, Box<Expr>),
    Call(Func, Box<Expr>),
}

impl Expr {
    pub fn eval(&self, x: f64) -> f64 {
        match self {
            Expr::Num(v) => *v,
            Expr::X => x,
            Expr::Pi => std::f64::consts::PI,
            Expr::E => std::f64::consts::E,
            Expr::Neg(a) => -a.eval(x),
            Expr::Add(a, b) => a.eval(x) + b.eval(x),
            Expr::Sub(a, b) => a.eval(x) - b.eval(x),
            Expr::Mul(a, b) => a.eval(x) * b.eval(x),
            Expr::Div(a, b) => a.eval(x) / b.eval(x),
            Expr::Pow(a, b) => pow(a.eval(x), b, x),
            Expr::Root(n, a) => {
                let (n, v) = (n.eval(x), a.eval(x));
                // Odd roots of negative numbers are real.
                if v < 0.0 && n.fract() == 0.0 && (n as i64) % 2 != 0 {
                    -(-v).powf(1.0 / n)
                } else {
                    v.powf(1.0 / n)
                }
            }
            Expr::Call(f, a) => f.apply(a.eval(x)),
        }
    }

    /// The expression in LaTeX (for labels).
    pub fn latex(&self) -> String {
        self.tex(0)
    }

    /// `prec`: 0 sum, 1 product, 2 power base.
    fn tex(&self, prec: u8) -> String {
        let paren = |s: String, need: bool| {
            if need {
                format!("\\left({s}\\right)")
            } else {
                s
            }
        };
        match self {
            Expr::Num(v) => paren(num(*v), *v < 0.0 && prec > 0),
            Expr::X => "x".into(),
            Expr::Pi => "\\pi".into(),
            Expr::E => "e".into(),
            Expr::Neg(a) => paren(format!("-{}", a.tex(1)), prec > 0),
            Expr::Add(a, b) => paren(format!("{} + {}", a.tex(0), b.tex(0)), prec > 0),
            Expr::Sub(a, b) => paren(format!("{} - {}", a.tex(0), b.tex(1)), prec > 0),
            Expr::Mul(a, b) => {
                let (l, r) = (a.tex(1), b.tex(1));
                // A number after something needs a dot: 2 \cdot 3, x \cdot 2.
                let dot = matches!(**b, Expr::Num(_)) || r.starts_with('-');
                paren(
                    format!("{l}{}{r}", if dot { " \\cdot " } else { " " }),
                    prec > 1,
                )
            }
            Expr::Div(a, b) => format!("\\frac{{{}}}{{{}}}", a.tex(0), b.tex(0)),
            Expr::Pow(a, b) => {
                if matches!(**a, Expr::E) {
                    return format!("e^{{{}}}", b.tex(0));
                }
                if let Expr::Call(f, arg) = &**a
                    && !matches!(f, Func::Sqrt | Func::Abs | Func::Exp)
                {
                    let call = f.latex(&arg.tex(0));
                    let name = format!("\\{}", f.name());
                    if let Some(rest) = call.strip_prefix(&name) {
                        return format!("{name}^{{{}}}{rest}", b.tex(0));
                    }
                }
                format!("{}^{{{}}}", a.tex(2), b.tex(0))
            }
            Expr::Root(n, a) => match **n {
                Expr::Num(2.0) => format!("\\sqrt{{{}}}", a.tex(0)),
                _ => format!("\\sqrt[{}]{{{}}}", n.tex(0), a.tex(0)),
            },
            Expr::Call(f, a) => f.latex(&a.tex(0)),
        }
    }
}

fn pow(base: f64, exp: &Expr, x: f64) -> f64 {
    // x^(1/3) of negative x: the real root when the exponent is p/q with
    // q odd.
    if base < 0.0
        && let Expr::Div(a, b) = exp
    {
        let (p, q) = (a.eval(x), b.eval(x));
        if p.fract() == 0.0 && q.fract() == 0.0 && (q as i64) % 2 != 0 {
            let r = (-base).powf(p / q);
            return if (p as i64) % 2 != 0 { -r } else { r };
        }
    }
    base.powf(exp.eval(x))
}

fn exp_depends_on_x(e: &Expr) -> bool {
    match e {
        Expr::X => true,
        Expr::Num(_) | Expr::Pi | Expr::E => false,
        Expr::Neg(a) | Expr::Call(_, a) => exp_depends_on_x(a),
        Expr::Add(a, b)
        | Expr::Sub(a, b)
        | Expr::Mul(a, b)
        | Expr::Div(a, b)
        | Expr::Pow(a, b)
        | Expr::Root(a, b) => exp_depends_on_x(a) || exp_depends_on_x(b),
    }
}

fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    /// A name: variable, constant or function (plain or `\name`).
    Name(String),
    Sym(char),
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() || ch == '$' {
            i += 1;
        } else if ch.is_ascii_digit()
            || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            let start = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            let s: String = c[start..i].iter().collect();
            out.push(Tok::Num(
                s.parse().map_err(|_| format!("bad number `{s}`"))?,
            ));
        } else if ch == '\\' {
            i += 1;
            let start = i;
            while i < c.len() && c[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i == start {
                // \, \; \! \  (spacing), \{ \}
                if let Some(&p) = c.get(i)
                    && (p == '{' || p == '}')
                {
                    out.push(Tok::Sym(p));
                }
                i += 1;
                continue;
            }
            let name: String = c[start..i].iter().collect();
            match name.as_str() {
                "left" | "right" | "big" | "Big" | "bigl" | "bigr" | "Bigl" | "Bigr" | "quad"
                | "qquad" | "displaystyle" => {}
                "cdot" | "times" => out.push(Tok::Sym('*')),
                "div" => out.push(Tok::Sym('/')),
                "lvert" | "rvert" | "vert" | "mid" => out.push(Tok::Sym('|')),
                "lbrace" => out.push(Tok::Sym('(')),
                "rbrace" => out.push(Tok::Sym(')')),
                "mathrm" | "operatorname" | "text" | "mathit" => {
                    // \mathrm{e}, \operatorname{sgn}: the group is a name.
                    if c.get(i) == Some(&'{') {
                        let end = c[i..].iter().position(|&x| x == '}').map(|p| i + p);
                        let Some(end) = end else {
                            return Err(format!("missing `}}` after \\{name}"));
                        };
                        let inner: String = c[i + 1..end].iter().collect();
                        out.extend(split_letters(inner.trim())?);
                        i = end + 1;
                    }
                }
                _ => out.push(Tok::Name(name)),
            }
        } else if ch.is_ascii_alphabetic() {
            let start = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            let word: String = c[start..i].iter().collect();
            // Digits only belong to log10 / log2; "x2" is x·2.
            let (letters, digits) = match word.find(|ch: char| ch.is_ascii_digit()) {
                Some(p) if !(word.starts_with("log") && p == 3) => {
                    (word[..p].to_string(), Some(word[p..].to_string()))
                }
                _ => (word, None),
            };
            out.extend(split_letters(&letters)?);
            if let Some(d) = digits {
                // "x2y" is not supported; keep it simple.
                out.push(Tok::Num(
                    d.parse().map_err(|_| format!("bad number `{d}`"))?,
                ));
            }
        } else if "+-*/^()[]{}|!_,".contains(ch) || ch == '·' || ch == '×' || ch == '−' {
            out.push(Tok::Sym(match ch {
                '·' | '×' => '*',
                '−' => '-',
                c => c,
            }));
            i += 1;
        } else {
            return Err(format!("unexpected `{ch}`"));
        }
    }
    Ok(out)
}

/// Splits a run of letters into names: known function names and
/// constants, else single letters ("xsin" → x, sin).
fn split_letters(w: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut rest = w;
    while !rest.is_empty() {
        let known = FUNCS
            .iter()
            .map(|(n, _)| *n)
            .chain(["pi", "frac", "dfrac", "tfrac", "sqrt"])
            .filter(|n| rest.starts_with(n))
            .max_by_key(|n| n.len());
        let take = known.map_or(1, |n| n.len());
        out.push(Tok::Name(rest[..take].to_string()));
        rest = &rest[take..];
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    abs_depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::Sym(c)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(match self.peek() {
                Some(t) => format!("expected `{c}` before {}", show(t)),
                None => format!("missing `{c}`"),
            })
        }
    }

    fn sum(&mut self) -> Result<Expr, String> {
        let mut a = self.term()?;
        loop {
            if self.eat('+') {
                a = Expr::Add(Box::new(a), Box::new(self.term()?));
            } else if self.eat('-') {
                a = Expr::Sub(Box::new(a), Box::new(self.term()?));
            } else {
                return Ok(a);
            }
        }
    }

    fn starts_atom(&self) -> bool {
        match self.peek() {
            Some(Tok::Num(_) | Tok::Name(_)) => true,
            Some(Tok::Sym('(' | '{' | '[')) => true,
            Some(Tok::Sym('|')) => self.abs_depth == 0,
            _ => false,
        }
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut a = self.unary()?;
        loop {
            if self.eat('*') {
                a = Expr::Mul(Box::new(a), Box::new(self.unary()?));
            } else if self.eat('/') {
                a = Expr::Div(Box::new(a), Box::new(self.unary()?));
            } else if self.starts_atom() {
                a = Expr::Mul(Box::new(a), Box::new(self.power()?));
            } else {
                return Ok(a);
            }
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat('-') {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat('+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Result<Expr, String> {
        let base = self.atom()?;
        if self.eat('^') {
            let e = self.exponent()?;
            return Ok(Expr::Pow(Box::new(base), Box::new(e)));
        }
        Ok(base)
    }

    /// After `^`: a braced group, or one (signed) atom with its own power.
    fn exponent(&mut self) -> Result<Expr, String> {
        if self.eat('{') {
            let e = self.sum()?;
            self.expect('}')?;
            return Ok(e);
        }
        if self.eat('-') {
            return Ok(Expr::Neg(Box::new(self.exponent()?)));
        }
        self.power()
    }

    fn group(&mut self) -> Result<Expr, String> {
        if self.eat('{') {
            let e = self.sum()?;
            self.expect('}')?;
            Ok(e)
        } else {
            // \frac12: single tokens
            self.atom()
        }
    }

    fn atom(&mut self) -> Result<Expr, String> {
        let Some(t) = self.peek().cloned() else {
            return Err("the expression ends too early".into());
        };
        self.pos += 1;
        match t {
            Tok::Num(v) => Ok(Expr::Num(v)),
            Tok::Sym('(') => {
                let e = self.sum()?;
                self.expect(')')?;
                Ok(e)
            }
            Tok::Sym('[') => {
                let e = self.sum()?;
                self.expect(']')?;
                Ok(e)
            }
            Tok::Sym('{') => {
                let e = self.sum()?;
                self.expect('}')?;
                Ok(e)
            }
            Tok::Sym('|') => {
                self.abs_depth += 1;
                let e = self.sum()?;
                self.abs_depth -= 1;
                self.expect('|')?;
                Ok(Expr::Call(Func::Abs, Box::new(e)))
            }
            Tok::Sym(c) => Err(format!("unexpected `{c}`")),
            Tok::Name(n) => self.name(&n),
        }
    }

    fn name(&mut self, n: &str) -> Result<Expr, String> {
        match n {
            "x" => return Ok(Expr::X),
            "e" => return Ok(Expr::E),
            "pi" => return Ok(Expr::Pi),
            "frac" | "dfrac" | "tfrac" => {
                let a = self.group()?;
                let b = self.group()?;
                return Ok(Expr::Div(Box::new(a), Box::new(b)));
            }
            "sqrt" => {
                let n = if self.eat('[') {
                    let n = self.sum()?;
                    self.expect(']')?;
                    n
                } else {
                    Expr::Num(2.0)
                };
                let a = if matches!(self.peek(), Some(Tok::Sym('('))) {
                    self.atom()?
                } else {
                    self.group()?
                };
                return Ok(Expr::Root(Box::new(n), Box::new(a)));
            }
            _ => {}
        }
        let Some(&(_, mut f)) = FUNCS.iter().find(|(name, _)| *name == n) else {
            return Err(if n.len() == 1 {
                format!("unknown variable `{n}`: the variable is x")
            } else {
                format!("unknown function `{n}`")
            });
        };
        // \log_{2}, \log_{10}
        if f == Func::Ln && self.eat('_') {
            let b = self.group()?.eval(0.0);
            f = if b == 2.0 {
                Func::Log2
            } else if b == 10.0 {
                Func::Log10
            } else {
                let arg = self.func_arg()?;
                return Ok(Expr::Div(
                    Box::new(Expr::Call(Func::Ln, Box::new(arg))),
                    Box::new(Expr::Num(b.ln())),
                ));
            };
        }
        // \sin^2 x = (\sin x)^2
        let power = if self.eat('^') {
            Some(self.exponent()?)
        } else {
            None
        };
        let arg = self.func_arg()?;
        let call = Expr::Call(f, Box::new(arg));
        Ok(match power {
            Some(p) => Expr::Pow(Box::new(call), Box::new(p)),
            None => call,
        })
    }

    /// The argument of a function: a bracketed group, or (as in `\sin 2x`)
    /// the product that follows, up to the next operator or function.
    fn func_arg(&mut self) -> Result<Expr, String> {
        if matches!(self.peek(), Some(Tok::Sym('(' | '{' | '['))) {
            // sin(x)^2: the power applies to the call.
            return self.atom();
        }
        let mut a = self.unary()?;
        while self.starts_atom()
            && !matches!(self.peek(), Some(Tok::Name(n)) if FUNCS.iter().any(|(f, _)| f == n))
        {
            a = Expr::Mul(Box::new(a), Box::new(self.power()?));
        }
        Ok(a)
    }
}

fn show(t: &Tok) -> String {
    match t {
        Tok::Num(v) => format!("`{}`", num(*v)),
        Tok::Name(n) => format!("`{n}`"),
        Tok::Sym(c) => format!("`{c}`"),
    }
}

/// Parses a function of x. A left side (`y =`, `f(x) =`) is ignored.
pub fn parse(src: &str) -> Result<Expr, String> {
    let rhs = src.rsplit('=').next().unwrap_or(src);
    let toks = lex(rhs)?;
    if toks.is_empty() {
        return Err("empty function".into());
    }
    let mut p = Parser {
        toks,
        pos: 0,
        abs_depth: 0,
    };
    let e = p.sum()?;
    match p.peek() {
        None => Ok(e),
        Some(t) => Err(format!("unexpected {} at the end", show(t))),
    }
}

/// Parses a number written as an expression: `-2pi`, `\pi/2`, `1e-3`
/// is not supported (write `0.001`).
pub fn number(src: &str) -> Result<f64, String> {
    let e = parse(src)?;
    if exp_depends_on_x(&e) {
        return Err(format!("`{src}` is not a number"));
    }
    let v = e.eval(0.0);
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!("`{src}` is not a finite number"))
    }
}

// ---- plots ---------------------------------------------------------------------

/// What to plot.
#[derive(Debug, Clone)]
pub struct Spec {
    /// One or more functions of x, separated by `;` or new lines.
    pub functions: String,
    pub x: (f64, f64),
    /// `None`: from the functions' values.
    pub y: Option<(f64, f64)>,
    /// Width and height of the plot area in pt.
    pub size: (f64, f64),
    /// Write each function next to its curve (default: when there are
    /// several).
    pub curve_labels: Option<bool>,
    pub x_label: String,
    pub y_label: String,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            functions: String::new(),
            x: (-5.0, 5.0),
            y: None,
            size: (200.0, 130.0),
            curve_labels: None,
            x_label: "$x$".into(),
            y_label: "$y$".into(),
        }
    }
}

/// The objects of a plot (one group), and notes about it.
#[derive(Debug, Clone)]
pub struct Plot {
    pub doc: Document,
    pub warnings: Vec<String>,
}

const COLORS: &[&str] = &[
    "blue", "red", "green", "orange", "purple", "brown", "magenta",
];
const AXIS_WIDTH: f64 = 0.6;
const CURVE_WIDTH: f64 = 1.0;
const TICK: f64 = 3.0;
const ARROW_LEN: f64 = 6.0;
const ARROW_WID: f64 = 4.0;
const OVERSHOOT: f64 = 10.0;
const TEXT: f64 = 8.0;
const SAMPLES: usize = 1200;

/// 1, 2 or 5 times a power of ten, near `raw`.
fn nice_step(raw: f64) -> f64 {
    let p = 10f64.powf(raw.log10().floor());
    let m = raw / p;
    let k = if m < 1.5 {
        1.0
    } else if m < 3.5 {
        2.0
    } else if m < 7.5 {
        5.0
    } else {
        10.0
    };
    k * p
}

fn tick_label(v: f64, step: f64) -> String {
    let decimals = (-step.log10().floor()).max(0.0) as usize;
    let s = format!("{v:.decimals$}");
    let s = if s
        .trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        "0".to_string()
    } else {
        s
    };
    format!("${s}$")
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[i]
}

/// Range of y covering the values (outliers near poles are cut off).
fn auto_y(values: &[f64]) -> Option<(f64, f64)> {
    let mut v: Vec<f64> = values.iter().copied().filter(|y| y.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let (lo, hi) = (v[0], v[v.len() - 1]);
    let (q1, q9) = (quantile(&v, 0.03), quantile(&v, 0.97));
    let (mut a, mut b) = if hi - lo <= 4.0 * (q9 - q1) + 1e-12 {
        (lo, hi)
    } else {
        let pad = 0.5 * (q9 - q1);
        (q1 - pad, q9 + pad)
    };
    if b - a < 1e-9 {
        a -= 1.0;
        b += 1.0;
    }
    Some((a, b))
}

/// Ramer–Douglas–Peucker simplification of a polyline (tolerance in pt).
fn simplify(p: &[Point], tol: f64) -> Vec<Point> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let mut keep = vec![false; p.len()];
    keep[0] = true;
    keep[p.len() - 1] = true;
    let mut stack = vec![(0, p.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (p[a], p[b]);
        let d = pb - pa;
        let len = d.hypot();
        let mut best = (0.0, a);
        for (i, q) in p.iter().enumerate().take(b).skip(a + 1) {
            let dist = if len < 1e-12 {
                (*q - pa).hypot()
            } else {
                (d.cross(*q - pa) / len).abs()
            };
            if dist > best.0 {
                best = (dist, i);
            }
        }
        if best.0 > tol {
            keep[best.1] = true;
            stack.push((a, best.1));
            stack.push((best.1, b));
        }
    }
    p.iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(q, _)| *q)
        .collect()
}

/// Clips segment a–b to the rectangle [0,w]×[0,h] (Liang–Barsky).
fn clip(a: Point, b: Point, w: f64, h: f64) -> Option<(Point, Point)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-d.x, a.x), (d.x, w - a.x), (-d.y, a.y), (d.y, h - a.y)] {
        if p.abs() < 1e-15 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1).then(|| (a + d * t0, a + d * t1))
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn d_of(subpaths: &[Vec<Point>]) -> String {
    let mut s = String::new();
    for sp in subpaths {
        for (i, p) in sp.iter().enumerate() {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(if i == 0 { "M " } else { "L " });
            s.push_str(&format!("{} {}", num(r2(p.x)), num(r2(p.y))));
        }
    }
    s
}

fn path(id: &str, d: String, color: &str, width: f64, fill: Option<&str>) -> Shape {
    Shape {
        id: Some(id.into()),
        kind: ShapeKind::Path,
        at: None,
        size: None,
        points: None,
        d: Some(d),
        rotation: None,
        class: None,
        color: Some(color.into()),
        fill: fill.map(Into::into),
        width: Some(width),
        layer: None,
    }
}

fn text(id: String, at: Point, content: String, anchor: Anchor, color: Option<&str>) -> Text {
    Text {
        id: Some(id),
        at: [r2(at.x), r2(at.y)],
        content,
        class: None,
        anchor: Some(anchor),
        size: Some(TEXT),
        rotation: None,
        color: color.map(Into::into),
        layer: None,
    }
}

/// Writes each function next to its curve, where it is farthest from the
/// other curves, the axes and the labels placed before.
fn place_curve_labels(
    curves: &[(usize, String, Vec<Vec<Point>>)],
    (w, h): (f64, f64),
    (ax_y, ay_x): (f64, f64),
    labels: &mut Vec<Text>,
) {
    // Dense points of every curve (the simplified polylines are sparse).
    let dense = |sp: &Vec<Point>| -> Vec<Point> {
        let mut out = Vec::new();
        for win in sp.windows(2) {
            let n = ((win[1] - win[0]).hypot() / 2.0).ceil().max(1.0) as usize;
            for k in 0..n {
                out.push(win[0].lerp(win[1], k as f64 / n as f64));
            }
        }
        out.extend(sp.last());
        out
    };
    let all: Vec<Vec<Point>> = curves
        .iter()
        .map(|(_, _, sps)| sps.iter().flat_map(dense).collect())
        .collect();
    // Rough label size: 5 pt per character of LaTeX, at most 50.
    let mut placed: Vec<Point> = Vec::new();
    for (ci, (n, content, _)) in curves.iter().enumerate() {
        let lw = (content.len() as f64 * 2.5).clamp(12.0, 50.0);
        let mut best: Option<(f64, Point)> = None;
        for p in all[ci].iter().step_by(3) {
            // The label sits above-right of p; it must fit in the plot.
            let c = Point::new(p.x + 2.0 + lw / 2.0, p.y + 7.0);
            if c.x + lw / 2.0 > w + 30.0 || c.y + 6.0 > h || c.y < 6.0 {
                continue;
            }
            let mut d = f64::INFINITY;
            for (cj, pts) in all.iter().enumerate() {
                if cj != ci {
                    for q in pts.iter().step_by(2) {
                        let dx = ((q.x - c.x).abs() - lw / 2.0).max(0.0);
                        let dy = ((q.y - c.y).abs() - 5.0).max(0.0);
                        d = d.min(dx.hypot(dy));
                    }
                }
            }
            // Own curve: it must not run through the label.
            for q in all[ci].iter().step_by(2) {
                let dx = ((q.x - c.x).abs() - lw / 2.0).max(0.0);
                let dy = ((q.y - c.y).abs() - 5.0).max(0.0);
                d = d.min(2.0 * dx.hypot(dy) + 1.0);
            }
            d = d.min(((c.y - ax_y).abs() - 5.0).max(0.0) * 1.5 + 0.5);
            d = d.min(((c.x - ay_x).abs() - lw / 2.0).max(0.0) * 1.5 + 0.5);
            for l in &placed {
                d = d.min(
                    (((l.x - c.x).abs() - lw).max(0.0)).hypot(((l.y - c.y).abs() - 10.0).max(0.0)),
                );
            }
            // Prefer the right part of the plot slightly.
            let score = d.min(25.0) + p.x / w;
            if best.is_none_or(|(b, _)| score > b) {
                best = Some((score, *p));
            }
        }
        let Some((_, p)) = best.or_else(|| all[ci].last().map(|p| (0.0, *p))) else {
            continue;
        };
        placed.push(Point::new(p.x + 2.0 + lw / 2.0, p.y + 7.0));
        labels.push(text(
            format!("plot_l{}", n + 1),
            p + crate::kurbo::Vec2::new(2.0, 2.0),
            content.clone(),
            Anchor::SouthWest,
            Some(COLORS[n % COLORS.len()]),
        ));
    }
}

/// Draws a plot from its settings; the group remembers them, so the plot
/// can be redrawn after they are edited.
pub fn from_source(src: &PlotSource) -> Result<Plot, String> {
    let n = |s: &str, what: &str| number(s).map_err(|e| format!("{what}: {e}"));
    let spec = Spec {
        functions: src.functions.clone(),
        x: (n(&src.x[0], "x from")?, n(&src.x[1], "x to")?),
        y: match &src.y {
            Some([a, b]) => Some((n(a, "y from")?, n(b, "y to")?)),
            None => None,
        },
        size: (src.size[0], src.size[1]),
        curve_labels: src.labels,
        x_label: src.x_label.clone().unwrap_or_else(|| "$x$".into()),
        y_label: src.y_label.clone().unwrap_or_else(|| "$y$".into()),
    };
    let mut p = plot(&spec)?;
    p.doc.groups[0].plot = Some(src.clone());
    Ok(p)
}

/// Draws the functions of `spec`. The plot area's lower-left corner is at
/// (0, 0).
pub fn plot(spec: &Spec) -> Result<Plot, String> {
    let mut warnings = Vec::new();
    let sources: Vec<&str> = spec
        .functions
        .split(['\n', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if sources.is_empty() {
        return Err("write a function of x, e.g. sin(x) or \\frac{1}{1+x^2}".into());
    }
    let funcs: Vec<(String, Expr)> = sources
        .iter()
        .map(|s| {
            parse(s)
                .map(|e| (s.to_string(), e))
                .map_err(|e| format!("{s}: {e}"))
        })
        .collect::<Result<_, _>>()?;
    let (x0, x1) = spec.x;
    if !(x0.is_finite() && x1.is_finite() && x1 > x0) {
        return Err("the x range must go from a smaller to a larger number".into());
    }
    let (w, h) = spec.size;
    if !(w > 1.0 && h > 1.0) {
        return Err("the plot size must be positive".into());
    }
    let xs: Vec<f64> = (0..=SAMPLES)
        .map(|i| x0 + (x1 - x0) * i as f64 / SAMPLES as f64)
        .collect();
    let values: Vec<Vec<f64>> = funcs
        .iter()
        .map(|(_, e)| xs.iter().map(|x| e.eval(*x)).collect())
        .collect();
    for ((src, _), v) in funcs.iter().zip(&values) {
        if v.iter().all(|y| !y.is_finite()) {
            warnings.push(format!(
                "{src} has no real values for x in [{}, {}]",
                num(x0),
                num(x1)
            ));
        }
    }
    let ystep;
    let (y0, y1) = match spec.y {
        Some((a, b)) if a.is_finite() && b.is_finite() && b > a => {
            ystep = nice_step((b - a) / 5.0);
            (a, b)
        }
        Some(_) => return Err("the y range must go from a smaller to a larger number".into()),
        None => {
            let all: Vec<f64> = values.iter().flatten().copied().collect();
            let (a, b) = auto_y(&all).unwrap_or((-1.0, 1.0));
            ystep = nice_step((b - a) / 5.0);
            ((a / ystep).floor() * ystep, (b / ystep).ceil() * ystep)
        }
    };
    let xstep = nice_step((x1 - x0) / 5.0);
    let px = |x: f64| (x - x0) / (x1 - x0) * w;
    let py = |y: f64| (y - y0) / (y1 - y0) * h;

    let mut doc = Document::default();
    let mut members = Vec::new();
    let mut add_shape = |doc: &mut Document, s: Shape| {
        members.push(s.id.clone().unwrap_or_default());
        doc.shapes.push(s);
    };

    // Axes: through zero when it is in range, else along the edges.
    let ax_y = if y0 <= 0.0 && 0.0 <= y1 { py(0.0) } else { 0.0 };
    let ay_x = if x0 <= 0.0 && 0.0 <= x1 { px(0.0) } else { 0.0 };
    let tip_x = Point::new(w + OVERSHOOT, ax_y);
    let tip_y = Point::new(ay_x, h + OVERSHOOT);
    add_shape(
        &mut doc,
        path(
            "plot_axes",
            format!(
                "M 0 {} L {} {} M {} 0 L {} {}",
                num(r2(ax_y)),
                num(r2(tip_x.x - ARROW_LEN / 2.0)),
                num(r2(ax_y)),
                num(r2(ay_x)),
                num(r2(ay_x)),
                num(r2(tip_y.y - ARROW_LEN / 2.0)),
            ),
            "black",
            AXIS_WIDTH,
            None,
        ),
    );
    let head = |tip: Point, dx: f64, dy: f64| {
        let base = Point::new(tip.x - dx * ARROW_LEN, tip.y - dy * ARROW_LEN);
        let n = (-dy * ARROW_WID / 2.0, dx * ARROW_WID / 2.0);
        format!(
            "M {} {} L {} {} L {} {} Z",
            num(r2(tip.x)),
            num(r2(tip.y)),
            num(r2(base.x + n.0)),
            num(r2(base.y + n.1)),
            num(r2(base.x - n.0)),
            num(r2(base.y - n.1)),
        )
    };
    add_shape(
        &mut doc,
        path(
            "plot_arrows",
            format!("{} {}", head(tip_x, 1.0, 0.0), head(tip_y, 0.0, 1.0)),
            "black",
            0.0,
            Some("black"),
        ),
    );

    // Ticks at multiples of the steps.
    let mut ticks = String::new();
    let mut labels: Vec<Text> = Vec::new();
    let both_zero = ax_y == py(0.0) && ay_x == px(0.0) && y0 <= 0.0 && x0 <= 0.0;
    let first = |a: f64, s: f64| (a / s - 1e-9).ceil() as i64;
    let last = |b: f64, s: f64| (b / s + 1e-9).floor() as i64;
    for k in first(x0, xstep)..=last(x1, xstep) {
        let v = k as f64 * xstep;
        let x = px(v);
        if (x - ay_x).abs() < 1e-6 && both_zero {
            continue;
        }
        ticks.push_str(&format!(
            "M {} {} L {} {} ",
            num(r2(x)),
            num(r2(ax_y - TICK / 2.0)),
            num(r2(x)),
            num(r2(ax_y + TICK / 2.0))
        ));
        labels.push(text(
            format!("plot_xt{}", labels.len() + 1),
            Point::new(x, ax_y - TICK / 2.0 - 1.5),
            tick_label(v, xstep),
            Anchor::North,
            None,
        ));
    }
    for k in first(y0, ystep)..=last(y1, ystep) {
        let v = k as f64 * ystep;
        let y = py(v);
        if (y - ax_y).abs() < 1e-6 && both_zero {
            continue;
        }
        ticks.push_str(&format!(
            "M {} {} L {} {} ",
            num(r2(ay_x - TICK / 2.0)),
            num(r2(y)),
            num(r2(ay_x + TICK / 2.0)),
            num(r2(y))
        ));
        labels.push(text(
            format!("plot_yt{}", labels.len() + 1),
            Point::new(ay_x - TICK / 2.0 - 1.5, y),
            tick_label(v, ystep),
            Anchor::East,
            None,
        ));
    }
    if both_zero {
        labels.push(text(
            "plot_origin".into(),
            Point::new(ay_x - 2.0, ax_y - 2.0),
            "$0$".into(),
            Anchor::NorthEast,
            None,
        ));
    }
    if !ticks.is_empty() {
        add_shape(
            &mut doc,
            path(
                "plot_ticks",
                ticks.trim_end().to_string(),
                "black",
                AXIS_WIDTH,
                None,
            ),
        );
    }
    if !spec.x_label.is_empty() {
        labels.push(text(
            "plot_xlabel".into(),
            tip_x + crate::kurbo::Vec2::new(2.0, 0.0),
            spec.x_label.clone(),
            Anchor::West,
            None,
        ));
    }
    if !spec.y_label.is_empty() {
        labels.push(text(
            "plot_ylabel".into(),
            tip_y + crate::kurbo::Vec2::new(0.0, 2.0),
            spec.y_label.clone(),
            Anchor::South,
            None,
        ));
    }

    // Curves.
    let show_labels = spec.curve_labels.unwrap_or(funcs.len() > 1);
    let mut curves: Vec<(usize, String, Vec<Vec<Point>>)> = Vec::new();
    for (n, ((src, expr), ys)) in funcs.iter().zip(&values).enumerate() {
        let color = COLORS[n % COLORS.len()];
        let mut subpaths: Vec<Vec<Point>> = Vec::new();
        let mut cur: Vec<Point> = Vec::new();
        let mut prev: Option<Point> = None;
        for (x, y) in xs.iter().zip(ys) {
            let p = Point::new(px(*x), py(*y));
            let ok = y.is_finite();
            match (prev, ok) {
                (Some(a), true) if (p.y - a.y).abs() <= 2.0 * h => match clip(a, p, w, h) {
                    Some((c0, c1)) => {
                        if cur.last().is_none_or(|l| (*l - c0).hypot() > 1e-9) {
                            if cur.len() > 1 {
                                subpaths.push(std::mem::take(&mut cur));
                            }
                            cur.clear();
                            cur.push(c0);
                        }
                        cur.push(c1);
                    }
                    None => {
                        if cur.len() > 1 {
                            subpaths.push(std::mem::take(&mut cur));
                        }
                        cur.clear();
                    }
                },
                _ => {
                    // A gap (no value) or a jump (pole): start again.
                    if cur.len() > 1 {
                        subpaths.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                }
            }
            prev = ok.then_some(p);
        }
        if cur.len() > 1 {
            subpaths.push(cur);
        }
        let subpaths: Vec<Vec<Point>> = subpaths.iter().map(|s| simplify(s, 0.05)).collect();
        if subpaths.is_empty() {
            warnings.push(format!("{src}: nothing to draw in this range"));
            continue;
        }
        add_shape(
            &mut doc,
            path(
                &format!("plot_f{}", n + 1),
                d_of(&subpaths),
                color,
                CURVE_WIDTH,
                None,
            ),
        );
        let content = if src.contains('\\') {
            let rhs = src
                .rsplit('=')
                .next()
                .unwrap_or(src)
                .trim()
                .trim_matches('$');
            format!("${rhs}$")
        } else {
            format!("${}$", expr.latex())
        };
        curves.push((n, content, subpaths));
    }
    if show_labels {
        place_curve_labels(&curves, (w, h), (ax_y, ay_x), &mut labels);
    }
    for t in labels {
        members.push(t.id.clone().unwrap_or_default());
        doc.texts.push(t);
    }
    doc.groups.push(Group {
        id: "plot".into(),
        members,
        latex: None,
        plot: None,
    });
    Ok(Plot { doc, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(src: &str, x: f64) -> f64 {
        parse(src).unwrap_or_else(|e| panic!("{src}: {e}")).eval(x)
    }

    #[test]
    fn plain_and_latex_agree() {
        let x = 0.7;
        let pairs = [
            ("sin(x)^2 + cos(x)^2", r"\sin^2 x + \cos^2 x"),
            ("exp(-x^2/2)", r"e^{-x^2/2}"),
            ("1/(1+x^2)", r"\frac{1}{1+x^2}"),
            ("sqrt(x)", r"\sqrt{x}"),
            ("abs(x - 1)", r"\left| x - 1 \right|"),
            ("2*pi*x", r"2\pi x"),
            ("x^3 - 2x", r"x^3 - 2 x"),
            ("ln(x)", r"\log x"),
            ("sin(2*x)", r"\sin 2x"),
            ("sin(x)*cos(x)", r"\sin x \cos x"),
            ("x^(1/3)", r"\sqrt[3]{x}"),
            ("tanh(x)", r"y = \tanh(x)"),
            ("-x^2", r"f(x) = -x^{2}"),
        ];
        for (a, b) in pairs {
            let (va, vb) = (at(a, x), at(b, x));
            assert!((va - vb).abs() < 1e-12, "{a} = {va}, {b} = {vb}");
        }
        assert!((at("sin(x)^2 + cos(x)^2", 1.3) - 1.0).abs() < 1e-12);
        assert_eq!(at("-x^2", 2.0), -4.0);
        assert_eq!(at("2^3^2", 0.0), 512.0);
        assert!((at(r"\sqrt[3]{x}", -8.0) + 2.0).abs() < 1e-12);
        assert!((at("x^(1/3)", -8.0) + 2.0).abs() < 1e-12);
        assert!((at("|x|", -3.0) - 3.0).abs() < 1e-12);
        assert!((at("2^x", 3.0) - 8.0).abs() < 1e-12);
    }

    #[test]
    fn errors_say_what_is_wrong() {
        assert!(parse("sin(t)").unwrap_err().contains("variable is x"));
        assert!(parse("foo(x)").is_err());
        assert!(parse("(x + 1").unwrap_err().contains(")"));
        assert!(parse("").is_err());
    }

    #[test]
    fn latex_of_plain_input() {
        assert_eq!(parse("1/(1+x^2)").unwrap().latex(), r"\frac{1}{1 + x^{2}}");
        assert_eq!(
            parse("sin(x)^2").unwrap().latex(),
            r"\sin^{2}\left(x\right)"
        );
        assert_eq!(parse("exp(-x)").unwrap().latex(), r"e^{-x}");
    }

    #[test]
    fn numbers_with_pi() {
        assert!((number("-2pi").unwrap() + 2.0 * std::f64::consts::PI).abs() < 1e-12);
        assert!((number(r"\pi/2").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(number("x").is_err());
    }

    #[test]
    fn plot_is_a_valid_group() {
        let p = plot(&Spec {
            functions: "sin(x); tan(x)".into(),
            x: (-6.0, 6.0),
            ..Spec::default()
        })
        .unwrap();
        let d = &p.doc;
        assert_eq!(d.groups.len(), 1);
        let n = d.shapes.len() + d.texts.len();
        assert_eq!(d.groups[0].members.len(), n);
        // tan has poles: several subpaths, and the y range is not blown up.
        let tan = d
            .shapes
            .iter()
            .find(|s| s.id.as_deref() == Some("plot_f2"))
            .unwrap();
        assert!(tan.d.as_ref().unwrap().matches('M').count() >= 3);
        let scene = crate::scene::build(d).expect("valid document");
        let b = scene.path_bounds().unwrap();
        assert!(b.y1 < 130.0 + 20.0 && b.y0 > -20.0, "{b:?}");
        // Curves are open paths.
        assert!(!d.shapes[2].d.as_ref().unwrap().contains('Z'));
    }

    #[test]
    fn y_range_and_ticks() {
        let p = plot(&Spec {
            functions: "x^2".into(),
            x: (-2.0, 2.0),
            ..Spec::default()
        })
        .unwrap();
        let labels: Vec<&str> = p.doc.texts.iter().map(|t| t.content.as_str()).collect();
        assert!(labels.contains(&"$4$"), "{labels:?}");
        assert!(labels.contains(&"$-2$"), "{labels:?}");
        assert!(labels.contains(&"$0$"));
    }

    #[test]
    fn sqrt_of_negative_x_leaves_a_gap() {
        let p = plot(&Spec {
            functions: "sqrt(x)".into(),
            x: (-1.0, 4.0),
            ..Spec::default()
        })
        .unwrap();
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        let none = plot(&Spec {
            functions: "sqrt(x)".into(),
            x: (-4.0, -1.0),
            ..Spec::default()
        })
        .unwrap();
        assert!(!none.warnings.is_empty());
    }
}
