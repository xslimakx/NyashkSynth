//! Tiny expression language for generating wavetable frames, e.g. `sin(2*pi*x + w*sin(4*pi*x))`.
//! Variables: `x` (phase 0..1), `w` (frame position 0..1), `pi`, `e`.

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f32),
    X,
    W,
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Tanh,
    Abs,
    Sqrt,
    Exp,
    Ln,
    Floor,
    Fract,
    Sign,
    Min,
    Max,
    Pow,
    Mod,
    Clamp,
    Saw,
    Tri,
    Sqr,
    Mix,
    Atan,
}

fn func(name: &str) -> Option<(Func, usize)> {
    Some(match name {
        "sin" => (Func::Sin, 1),
        "cos" => (Func::Cos, 1),
        "tan" => (Func::Tan, 1),
        "tanh" => (Func::Tanh, 1),
        "atan" => (Func::Atan, 1),
        "abs" => (Func::Abs, 1),
        "sqrt" => (Func::Sqrt, 1),
        "exp" => (Func::Exp, 1),
        "ln" | "log" => (Func::Ln, 1),
        "floor" => (Func::Floor, 1),
        "fract" => (Func::Fract, 1),
        "sign" => (Func::Sign, 1),
        "min" => (Func::Min, 2),
        "max" => (Func::Max, 2),
        "pow" => (Func::Pow, 2),
        "mod" => (Func::Mod, 2),
        "clamp" => (Func::Clamp, 3),
        "mix" => (Func::Mix, 3),
        "saw" => (Func::Saw, 1),
        "tri" => (Func::Tri, 1),
        "sqr" => (Func::Sqr, 1),
        _ => return None,
    })
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn expr(&mut self, depth: usize) -> Result<Expr, String> {
        if depth > 40 {
            return Err("expression too deeply nested".into());
        }
        let mut l = self.term(depth)?;
        while let Some(c) = self.peek() {
            if c == b'+' || c == b'-' {
                self.i += 1;
                let r = self.term(depth)?;
                l = Expr::Bin(c as char, Box::new(l), Box::new(r));
            } else {
                break;
            }
        }
        Ok(l)
    }
    fn term(&mut self, depth: usize) -> Result<Expr, String> {
        let mut l = self.unary(depth)?;
        while let Some(c) = self.peek() {
            if c == b'*' || c == b'/' || c == b'%' {
                self.i += 1;
                let r = self.unary(depth)?;
                l = Expr::Bin(c as char, Box::new(l), Box::new(r));
            } else {
                break;
            }
        }
        Ok(l)
    }
    fn unary(&mut self, depth: usize) -> Result<Expr, String> {
        match self.peek() {
            Some(b'-') => {
                self.i += 1;
                Ok(Expr::Neg(Box::new(self.unary(depth + 1)?)))
            }
            Some(b'+') => {
                self.i += 1;
                self.unary(depth + 1)
            }
            _ => self.power(depth),
        }
    }
    fn power(&mut self, depth: usize) -> Result<Expr, String> {
        let base = self.atom(depth)?;
        if self.peek() == Some(b'^') {
            self.i += 1;
            let e = self.unary(depth + 1)?;
            return Ok(Expr::Bin('^', Box::new(base), Box::new(e)));
        }
        Ok(base)
    }
    fn atom(&mut self, depth: usize) -> Result<Expr, String> {
        match self.peek() {
            None => Err("unexpected end".into()),
            Some(b'(') => {
                self.i += 1;
                let e = self.expr(depth + 1)?;
                if self.peek() != Some(b')') {
                    return Err("missing )".into());
                }
                self.i += 1;
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[st..self.i]).unwrap_or("").parse::<f32>().map(Expr::Num).map_err(|_| "bad number".to_string())
            }
            Some(c) if c.is_ascii_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_') {
                    self.i += 1;
                }
                let name = std::str::from_utf8(&self.s[st..self.i]).unwrap_or("").to_ascii_lowercase();
                if self.peek() == Some(b'(') {
                    let Some((f, n)) = func(&name) else { return Err(format!("unknown function {name}")) };
                    self.i += 1;
                    let mut args = Vec::new();
                    loop {
                        args.push(self.expr(depth + 1)?);
                        match self.peek() {
                            Some(b',') => self.i += 1,
                            Some(b')') => {
                                self.i += 1;
                                break;
                            }
                            _ => return Err("missing )".into()),
                        }
                    }
                    if args.len() != n {
                        return Err(format!("{name} needs {n} argument(s)"));
                    }
                    return Ok(Expr::Call(f, args));
                }
                match name.as_str() {
                    "x" | "t" | "phase" => Ok(Expr::X),
                    "w" | "frame" | "pos" => Ok(Expr::W),
                    "pi" => Ok(Expr::Num(std::f32::consts::PI)),
                    "tau" => Ok(Expr::Num(std::f32::consts::TAU)),
                    "e" => Ok(Expr::Num(std::f32::consts::E)),
                    _ => Err(format!("unknown name {name}")),
                }
            }
            Some(c) => Err(format!("unexpected '{}'", c as char)),
        }
    }
}

pub fn parse(src: &str) -> Result<Expr, String> {
    if src.len() > 400 {
        return Err("formula too long".into());
    }
    let mut p = P { s: src.as_bytes(), i: 0 };
    let e = p.expr(0)?;
    if p.peek().is_some() {
        return Err(format!("unexpected '{}'", p.s[p.i] as char));
    }
    Ok(e)
}

impl Expr {
    pub fn eval(&self, x: f32, w: f32) -> f32 {
        use std::f32::consts::{PI, TAU};
        let v = match self {
            Expr::Num(n) => *n,
            Expr::X => x,
            Expr::W => w,
            Expr::Neg(a) => -a.eval(x, w),
            Expr::Bin(op, a, b) => {
                let (a, b) = (a.eval(x, w), b.eval(x, w));
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => {
                        if b.abs() < 1e-9 {
                            0.0
                        } else {
                            a / b
                        }
                    }
                    '%' => {
                        if b.abs() < 1e-9 {
                            0.0
                        } else {
                            a.rem_euclid(b)
                        }
                    }
                    _ => a.signum() * a.abs().powf(b.clamp(-16.0, 16.0)),
                }
            }
            Expr::Call(f, a) => {
                let g = |i: usize| a[i].eval(x, w);
                match f {
                    Func::Sin => g(0).sin(),
                    Func::Cos => g(0).cos(),
                    Func::Tan => g(0).tan().clamp(-50.0, 50.0),
                    Func::Tanh => g(0).tanh(),
                    Func::Atan => g(0).atan(),
                    Func::Abs => g(0).abs(),
                    Func::Sqrt => g(0).max(0.0).sqrt(),
                    Func::Exp => g(0).clamp(-30.0, 30.0).exp(),
                    Func::Ln => g(0).max(1e-9).ln(),
                    Func::Floor => g(0).floor(),
                    Func::Fract => g(0).rem_euclid(1.0),
                    Func::Sign => g(0).signum(),
                    Func::Min => g(0).min(g(1)),
                    Func::Max => g(0).max(g(1)),
                    Func::Pow => g(0).signum() * g(0).abs().powf(g(1).clamp(-16.0, 16.0)),
                    Func::Mod => {
                        let b = g(1);
                        if b.abs() < 1e-9 {
                            0.0
                        } else {
                            g(0).rem_euclid(b)
                        }
                    }
                    Func::Clamp => g(0).clamp(g(1).min(g(2)), g(1).max(g(2))),
                    Func::Mix => {
                        let t = g(2);
                        g(0) * (1.0 - t) + g(1) * t
                    }
                    Func::Saw => 2.0 * g(0).rem_euclid(1.0) - 1.0,
                    Func::Tri => {
                        let p = g(0).rem_euclid(1.0);
                        1.0 - 4.0 * (p - 0.5).abs()
                    }
                    Func::Sqr => {
                        if g(0).rem_euclid(1.0) < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                }
            }
        };
        let _ = (PI, TAU);
        if v.is_finite() {
            v.clamp(-64.0, 64.0)
        } else {
            0.0
        }
    }
}

/// Evaluates one frame of `n` samples.
pub fn frame(e: &Expr, w: f32, n: usize) -> Vec<f32> {
    (0..n).map(|i| e.eval(i as f32 / n as f32, w)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_evaluates() {
        let e = parse("sin(2*pi*x)").unwrap();
        assert!((e.eval(0.25, 0.0) - 1.0).abs() < 1e-5);
        let e = parse("-x^2 + w*2").unwrap();
        assert!((e.eval(0.5, 0.5) - 0.75).abs() < 1e-5);
        let e = parse("mix(saw(x), sqr(x), w)").unwrap();
        assert!((e.eval(0.25, 0.0) + 0.5).abs() < 1e-5);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("sin(").is_err());
        assert!(parse("foo(x)").is_err());
        assert!(parse("1 +").is_err());
        assert!(parse("min(1)").is_err());
        assert!(parse("1 2").is_err());
        assert_eq!(parse("1/0").unwrap().eval(0.0, 0.0), 0.0);
    }
}
