//! Shared canonical compact JSON encoding used for persistence admission.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Serialization,
}

/// Encodes one JSON value with recursively UTF-8-byte-sorted object keys.
pub fn encode(value: &Value) -> Vec<u8> {
    let mut output = Vec::new();
    append(value, &mut output);
    output
}

/// Encodes a serializable projection as one canonical JSON Line, including LF.
pub fn encode_line<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let value = serde_json::to_value(value).map_err(|_| Error::Serialization)?;
    let mut output = encode(&value);
    output.push(b'\n');
    Ok(output)
}

/// Returns the exact canonical JSON-Line byte size without exposing its bytes.
pub fn line_size<T: Serialize>(value: &T) -> Result<usize, Error> {
    Ok(encode_line(value)?.len())
}

fn append(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(boolean) => output.extend_from_slice(if *boolean { b"true" } else { b"false" }),
        Value::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        Value::String(string) => append_string(string, output),
        Value::Array(values) => {
            output.push(b'[');
            for (index, item) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                append(item, output);
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|(left, _), (right, _)| left.as_bytes().cmp(right.as_bytes()));
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                append_string(key, output);
                output.push(b':');
                append(item, output);
            }
            output.push(b'}');
        }
    }
}

fn append_string(value: &str, output: &mut Vec<u8>) {
    output.push(b'"');
    for character in value.chars() {
        match character {
            '"' => output.extend_from_slice(b"\\\""),
            '\\' => output.extend_from_slice(b"\\\\"),
            '\u{08}' => output.extend_from_slice(b"\\b"),
            '\t' => output.extend_from_slice(b"\\t"),
            '\n' => output.extend_from_slice(b"\\n"),
            '\u{0c}' => output.extend_from_slice(b"\\f"),
            '\r' => output.extend_from_slice(b"\\r"),
            character if character <= '\u{1f}' => {
                output.extend_from_slice(format!("\\u{:04x}", character as u32).as_bytes());
            }
            character => {
                let mut buffer = [0; 4];
                output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    output.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_encoder_sorts_every_object_and_obeys_required_escapes() {
        let value = json!({"z": {"é": "\n", "a": "\u{0001}"}, "a": ["/", "\""]});
        assert_eq!(
            String::from_utf8(encode(&value)).unwrap(),
            "{\"a\":[\"/\",\"\\\"\"],\"z\":{\"a\":\"\\u0001\",\"é\":\"\\n\"}}"
        );
        assert_eq!(encode_line(&value).unwrap().last(), Some(&b'\n'));
    }
}
