use super::parser::{BinOp, Expr};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct EvalError(pub String);

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Expression evaluation error: {}", self.0)
    }
}

pub fn eval(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, EvalError> {
    match expr {
        Expr::Null => Ok(Value::Null),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Number(n) => Ok(Value::from(*n)),
        Expr::Str(s) => Ok(Value::String(s.clone())),

        Expr::Var(path) => {
            let root = path[0].as_str();
            let val = vars
                .get(root)
                .ok_or_else(|| EvalError(format!("Unknown variable: '{}'", root)))?;

            let mut cur = val;
            for seg in &path[1..] {
                cur = cur
                    .get(seg)
                    .ok_or_else(|| EvalError(format!("No field '{}' on value", seg)))?;
            }
            Ok(cur.clone())
        }

        Expr::UnaryMinus(inner) => {
            let v = eval(inner, vars)?;
            let n = as_f64(&v)?;
            Ok(Value::from(-n))
        }

        Expr::Not(inner) => {
            let v = eval(inner, vars)?;
            let b = as_bool(&v)?;
            Ok(Value::Bool(!b))
        }

        Expr::BinOp(left, op, right) => eval_binop(left, op, right, vars),
    }
}

fn eval_binop(
    left: &Expr,
    op: &BinOp,
    right: &Expr,
    vars: &HashMap<String, Value>,
) -> Result<Value, EvalError> {
    match op {
        BinOp::And => {
            let l = eval(left, vars)?;
            if !as_bool(&l)? {
                return Ok(Value::Bool(false));
            }
            let r = eval(right, vars)?;
            Ok(Value::Bool(as_bool(&r)?))
        }
        BinOp::Or => {
            let l = eval(left, vars)?;
            if as_bool(&l)? {
                return Ok(Value::Bool(true));
            }
            let r = eval(right, vars)?;
            Ok(Value::Bool(as_bool(&r)?))
        }
        _ => {
            let l = eval(left, vars)?;
            let r = eval(right, vars)?;
            match op {
                BinOp::Eq => Ok(Value::Bool(values_equal(&l, &r))),
                BinOp::Ne => Ok(Value::Bool(!values_equal(&l, &r))),
                BinOp::Lt => Ok(Value::Bool(compare_numeric(&l, &r)? < 0.0)),
                BinOp::Le => Ok(Value::Bool(compare_numeric(&l, &r)? <= 0.0)),
                BinOp::Gt => Ok(Value::Bool(compare_numeric(&l, &r)? > 0.0)),
                BinOp::Ge => Ok(Value::Bool(compare_numeric(&l, &r)? >= 0.0)),
                BinOp::Add => Ok(Value::from(as_f64(&l)? + as_f64(&r)?)),
                BinOp::Sub => Ok(Value::from(as_f64(&l)? - as_f64(&r)?)),
                BinOp::Mul => Ok(Value::from(as_f64(&l)? * as_f64(&r)?)),
                BinOp::Div => {
                    let d = as_f64(&r)?;
                    if d == 0.0 {
                        return Err(EvalError("Division by zero".into()));
                    }
                    Ok(Value::from(as_f64(&l)? / d))
                }
                BinOp::And | BinOp::Or => unreachable!(),
            }
        }
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    // For numeric comparison, compare as f64 to handle integer/float mixed types.
    if let (Some(na), Some(nb)) = (a.as_f64(), b.as_f64()) {
        return (na - nb).abs() < f64::EPSILON;
    }
    a == b
}

fn compare_numeric(a: &Value, b: &Value) -> Result<f64, EvalError> {
    let na = as_f64(a)?;
    let nb = as_f64(b)?;
    Ok(na - nb)
}

fn as_f64(v: &Value) -> Result<f64, EvalError> {
    v.as_f64()
        .ok_or_else(|| EvalError(format!("Expected number, got {:?}", v)))
}

fn as_bool(v: &Value) -> Result<bool, EvalError> {
    v.as_bool()
        .ok_or_else(|| EvalError(format!("Expected boolean, got {:?}", v)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::parser::parse;

    fn vars(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn numeric_comparison_true() {
        let e = parse("= amount > 100").unwrap();
        let v = vars(&[("amount", Value::from(150))]);
        assert_eq!(eval(&e, &v).unwrap(), Value::Bool(true));
    }

    #[test]
    fn numeric_comparison_false() {
        let e = parse("= amount > 100").unwrap();
        let v = vars(&[("amount", Value::from(50))]);
        assert_eq!(eval(&e, &v).unwrap(), Value::Bool(false));
    }

    #[test]
    fn string_equality() {
        let e = parse(r#"= status = "active""#).unwrap();
        let v = vars(&[("status", Value::String("active".into()))]);
        assert_eq!(eval(&e, &v).unwrap(), Value::Bool(true));
    }

    #[test]
    fn unknown_variable() {
        let e = parse("unknown > 1").unwrap();
        assert!(eval(&e, &HashMap::new()).is_err());
    }

    #[test]
    fn boolean_and_or() {
        let e = parse("true and false or true").unwrap();
        assert_eq!(eval(&e, &HashMap::new()).unwrap(), Value::Bool(true));
    }

    #[test]
    fn arithmetic() {
        let e = parse("2 + 3 * 4").unwrap();
        // 3*4=12, 2+12=14 — but our parser is left-associative at each precedence level
        // actually mul is higher than add, so: 2 + (3 * 4) = 14
        let result = eval(&e, &HashMap::new()).unwrap();
        assert_eq!(result.as_f64().unwrap(), 14.0);
    }

    #[test]
    fn member_access() {
        let e = parse("order.amount").unwrap();
        let order = serde_json::json!({"amount": 200});
        let v = vars(&[("order", order)]);
        assert_eq!(eval(&e, &v).unwrap(), Value::from(200));
    }

    #[test]
    fn unary_minus() {
        let e = parse("-5 < 0").unwrap();
        assert_eq!(eval(&e, &HashMap::new()).unwrap(), Value::Bool(true));
    }
}
