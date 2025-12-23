// src/transform/functions.rs
//! Custom VRL stdlib functions for WASM compatibility
//! These replace VRL's stdlib which depends on zstd (C code)

use vrl::compiler::prelude::*;
use vrl::value::Value;

/// Get all custom functions
pub fn all() -> Vec<Box<dyn Function>> {
    vec![
        Box::new(ToInt),
        Box::new(ToString_),
        Box::new(EncodeJson),
        Box::new(Get),
        Box::new(IsEmpty),
        Box::new(IsObject),
        Box::new(IsArray),
        Box::new(Floor),
    ]
}

// --- to_int ---
#[derive(Clone, Copy, Debug)]
pub struct ToInt;

impl Function for ToInt {
    fn identifier(&self) -> &'static str {
        "to_int"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(ToIntFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }
}

#[derive(Debug, Clone)]
struct ToIntFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToIntFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        match value {
            Value::Integer(i) => Ok(Value::Integer(i)),
            Value::Float(f) => {
                let float_val = f.into_inner();
                if float_val.is_nan() {
                    return Err("cannot convert NaN to int".into());
                }
                if float_val.is_infinite() {
                    return Err("cannot convert infinity to int".into());
                }
                // i64::MAX (9223372036854775807) cannot be exactly represented as f64;
                // it rounds to 9223372036854775808.0. Use the largest safe f64 value.
                const MAX_SAFE_FLOAT: f64 = 9_223_372_036_854_774_784.0;
                const MIN_SAFE_FLOAT: f64 = i64::MIN as f64; // -9223372036854775808.0 is exact
                if !(MIN_SAFE_FLOAT..=MAX_SAFE_FLOAT).contains(&float_val) {
                    return Err(format!("float {} is out of range for i64", float_val).into());
                }
                Ok(Value::Integer(float_val as i64))
            }
            Value::Bytes(b) => {
                let s = String::from_utf8_lossy(&b);
                s.trim()
                    .parse::<i64>()
                    .map(Value::Integer)
                    .map_err(|_| "failed to parse int".into())
            }
            _ => Err("cannot convert to int".into()),
        }
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::integer().fallible()
    }
}

// --- to_string ---
#[derive(Clone, Copy, Debug)]
pub struct ToString_;

impl Function for ToString_ {
    fn identifier(&self) -> &'static str {
        "to_string"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(ToStringFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }
}

#[derive(Debug, Clone)]
struct ToStringFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToStringFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let s = match value {
            Value::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Null => "null".to_string(),
            _ => return Err("cannot convert to string".into()),
        };
        Ok(Value::Bytes(s.into()))
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

// --- encode_json ---
#[derive(Clone, Copy, Debug)]
pub struct EncodeJson;

impl Function for EncodeJson {
    fn identifier(&self) -> &'static str {
        "encode_json"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(EncodeJsonFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }
}

#[derive(Debug, Clone)]
struct EncodeJsonFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for EncodeJsonFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let json = crate::convert::vrl_value_to_json_lossy(&value);
        Ok(Value::Bytes(json.to_string().into()))
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

// --- get ---
#[derive(Clone, Copy, Debug)]
pub struct Get;

impl Function for Get {
    fn identifier(&self) -> &'static str {
        "get"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::OBJECT,
                required: true,
            },
            Parameter {
                keyword: "path",
                kind: kind::ARRAY,
                required: true,
            },
        ]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let path = arguments.required("path");
        Ok(GetFn { value, path }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }
}

#[derive(Debug, Clone)]
struct GetFn {
    value: Box<dyn Expression>,
    path: Box<dyn Expression>,
}

impl FunctionExpression for GetFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let path = self.path.resolve(ctx)?;

        let path_arr = match path {
            Value::Array(arr) => arr,
            _ => return Err("path must be array".into()),
        };

        let mut current = value;
        for segment in path_arr.iter() {
            let key = match segment {
                Value::Bytes(b) => String::from_utf8_lossy(b).to_string(),
                Value::Integer(i) => i.to_string(),
                _ => return Err("path segment must be string or int".into()),
            };

            match current {
                Value::Object(map) => {
                    let key_string: KeyString = key.as_str().into();
                    current = map.get(&key_string).cloned().unwrap_or(Value::Null);
                }
                Value::Array(arr) => {
                    let idx: usize = key.parse().map_err(|_| "invalid array index")?;
                    current = arr.get(idx).cloned().unwrap_or(Value::Null);
                }
                _ => return Ok(Value::Null),
            }
        }

        Ok(current)
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::any().fallible()
    }
}

// --- is_empty ---
#[derive(Clone, Copy, Debug)]
pub struct IsEmpty;

impl Function for IsEmpty {
    fn identifier(&self) -> &'static str {
        "is_empty"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(IsEmptyFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }
}

#[derive(Debug, Clone)]
struct IsEmptyFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsEmptyFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let is_empty = match value {
            Value::Bytes(b) => b.is_empty(),
            Value::Array(arr) => arr.is_empty(),
            Value::Object(map) => map.is_empty(),
            Value::Null => true,
            _ => false,
        };
        Ok(Value::Boolean(is_empty))
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

// --- is_object ---
#[derive(Clone, Copy, Debug)]
pub struct IsObject;

impl Function for IsObject {
    fn identifier(&self) -> &'static str {
        "is_object"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(IsObjectFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }
}

#[derive(Debug, Clone)]
struct IsObjectFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsObjectFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        Ok(Value::Boolean(matches!(value, Value::Object(_))))
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

// --- is_array ---
#[derive(Clone, Copy, Debug)]
pub struct IsArray;

impl Function for IsArray {
    fn identifier(&self) -> &'static str {
        "is_array"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(IsArrayFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }
}

#[derive(Debug, Clone)]
struct IsArrayFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsArrayFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        Ok(Value::Boolean(matches!(value, Value::Array(_))))
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

// --- floor ---
#[derive(Clone, Copy, Debug)]
pub struct Floor;

impl Function for Floor {
    fn identifier(&self) -> &'static str {
        "floor"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(FloorFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }
}

#[derive(Debug, Clone)]
struct FloorFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for FloorFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        match value {
            Value::Integer(i) => Ok(Value::Integer(i)),
            Value::Float(f) => {
                let floored = f.into_inner().floor();
                // Convert to integer after flooring
                Ok(Value::Integer(floored as i64))
            }
            _ => Err("floor requires a numeric value".into()),
        }
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::integer().fallible()
    }
}
