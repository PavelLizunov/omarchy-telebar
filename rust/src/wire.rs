use serde_json::{Map, Value, json};

pub const LINE_MAX: usize = 64 * 1024;
pub const OUTBOX_MAX: usize = 16 * 1024 * 1024;
pub const CLIENTS_MAX: usize = 16;
pub const IPC_PENDING_MAX: usize = 512;
pub const TD_PENDING_MAX: usize = 1024;

pub struct Request {
    pub id: Value,
    pub command: String,
    pub args: Map<String, Value>,
    pub account: Value,
}

pub fn failure(id: Value, reason: &str, code: i64) -> Value {
    json!({"id": id, "ok": false, "error": reason, "code": code})
}

pub fn success(id: Value, result: Value) -> Value {
    json!({"id": id, "ok": true, "result": result})
}

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.to_string().parse::<f64>().is_ok_and(|n| n != 0.0),
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}

pub fn is_integer(value: &Value) -> bool {
    value.as_number().is_some_and(|n| {
        let s = n.to_string();
        !s.contains(['.', 'e', 'E'])
    })
}

pub fn bounded_json(
    bytes: &[u8],
    depth: usize,
    items: usize,
    string: usize,
) -> Result<Value, &'static str> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "not a JSON request")?;
    let mut stack = vec![(&value, 1)];
    let mut count = 0usize;
    while let Some((node, level)) = stack.pop() {
        if level > depth {
            return Err("not a JSON request");
        }
        match node {
            Value::Object(map) => {
                count = count.saturating_add(map.len());
                for (key, value) in map {
                    if key.chars().count() > string {
                        return Err("not a JSON request");
                    }
                    stack.push((value, level + 1));
                }
            }
            Value::Array(array) => {
                count = count.saturating_add(array.len());
                stack.extend(array.iter().map(|value| (value, level + 1)));
            }
            Value::String(text) if text.chars().count() > string => {
                return Err("not a JSON request");
            }
            _ => {}
        }
        if count > items {
            return Err("not a JSON request");
        }
    }
    Ok(value)
}

impl Request {
    pub fn parse(bytes: &[u8]) -> Result<Self, Value> {
        let value = bounded_json(bytes, 8, 1024, 16 * 1024)
            .map_err(|reason| failure(Value::Null, reason, 0))?;
        let Value::Object(mut map) = value else {
            return Err(failure(Value::Null, "not a request", 0));
        };
        let id = map.remove("id").unwrap_or(Value::Null);
        if !is_integer(&id) {
            return Err(failure(Value::Null, "id must be an integer", 0));
        }
        let command = map
            .remove("cmd")
            .and_then(|v| v.as_str().map(str::to_owned));
        let args = map.remove("args").unwrap_or_else(|| json!({}));
        let (Some(command), Value::Object(args)) = (command, args) else {
            return Err(failure(id, "unknown command", 0));
        };
        let account = args
            .get("account")
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or_else(|| map.remove("account").unwrap_or(Value::Null));
        Ok(Self {
            id,
            command,
            args,
            account,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_unicode_and_envelopes_keep_errors() {
        for (raw, error) in [
            (r#"{"id":true}"#, "id must be an integer"),
            (r#"{"id":1.0}"#, "id must be an integer"),
            (r#"[]"#, "not a request"),
            (r#"{"id":1,"cmd":"hello","args":null}"#, "unknown command"),
            (r#"{"id":1,"cmd":"\ud800"}"#, "not a JSON request"),
        ] {
            assert_eq!(
                Request::parse(raw.as_bytes()).err().unwrap()["error"],
                error
            );
        }
    }

    #[test]
    fn routing_preserves_null_and_falsy_top_level() {
        let req =
            Request::parse(br#"{"id":1,"cmd":"hello","account":false,"args":{"account":""}}"#)
                .unwrap_or_else(|_| panic!("valid request"));
        assert_eq!(req.account, false);
        let req = Request::parse(br#"{"id":184467440737095516160,"cmd":"hello","account":"default","args":{"account":"other"}}"#).unwrap_or_else(|_| panic!("valid request"));
        assert_eq!(req.account, "other");
        assert_eq!(req.id.to_string(), "184467440737095516160");
    }

    #[test]
    fn depth_and_character_limits_are_not_byte_limits() {
        let input = json!({"text": "😀".repeat(16 * 1024)}).to_string();
        assert!(bounded_json(input.as_bytes(), 8, 1024, 16 * 1024).is_ok());
        assert!(bounded_json(b"[[[[[[[[]]]]]]]]", 8, 1024, 16 * 1024).is_ok());
        assert!(bounded_json(b"[[[[[[[[[]]]]]]]]]", 8, 1024, 16 * 1024).is_err());
    }
}
