//! Strict, closed version 2 protocol and transport-independent dispatch.

use crate::{
    application::Core,
    applications::ApplicationResolver,
    canonical_json,
    error::{Error, Result},
};
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::{collections::BTreeSet, fmt};
use uuid::Uuid;

pub const API_VERSION: u64 = 2;
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WireError {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}
impl From<Error> for WireError {
    fn from(error: Error) -> Self {
        Self {
            code: error.0.into(),
            field: None,
        }
    }
}
impl WireError {
    pub(crate) fn valid(&self) -> bool {
        !self.code.is_empty()
            && self.code.len() <= 128
            && self
                .code
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._".contains(&b))
            && self.field.as_ref().is_none_or(|f| {
                f.len() <= 128
                    && f.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._[]".contains(&b))
            })
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub protocol_version: u64,
    pub request_id: Option<Uuid>,
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WireError>,
}
impl Response {
    pub fn failure(id: Option<Uuid>, error: Error) -> Self {
        Self {
            protocol_version: API_VERSION,
            request_id: id,
            kind: "response",
            result: None,
            error: Some(error.into()),
        }
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        let bytes = canonical_json::encode_line(self).map_err(|_| Error("internal.error"))?;
        if bytes.len() > MAX_LINE_BYTES {
            return canonical_json::encode_line(&Self::failure(
                self.request_id,
                Error("limit.exceeded"),
            ))
            .map_err(|_| Error("internal.error"));
        }
        Ok(bytes)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Projects,
    Templates,
    Create {
        base_name: String,
        template_name: String,
    },
    Sessions,
    Start {
        project_name: String,
    },
    Switch {
        session_id: Uuid,
    },
    Deactivate,
    Stop {
        session_id: Uuid,
    },
    Applications,
}
impl Command {
    pub fn mutates(&self) -> bool {
        matches!(
            self,
            Self::Create { .. }
                | Self::Start { .. }
                | Self::Switch { .. }
                | Self::Deactivate
                | Self::Stop { .. }
        )
    }
    pub fn request(&self, request_id: Uuid) -> Value {
        let (method, params) = match self {
            Self::Projects => ("project.list", serde_json::json!({})),
            Self::Templates => ("template.list", serde_json::json!({})),
            Self::Create {
                base_name,
                template_name,
            } => (
                "project.create",
                serde_json::json!({"base_name":base_name,"template_name":template_name}),
            ),
            Self::Sessions => ("session.list", serde_json::json!({})),
            Self::Start { project_name } => (
                "session.start",
                serde_json::json!({"project_name":project_name}),
            ),
            Self::Switch { session_id } => (
                "session.switch",
                serde_json::json!({"session_id":session_id}),
            ),
            Self::Deactivate => ("session.deactivate", serde_json::json!({})),
            Self::Stop { session_id } => {
                ("session.stop", serde_json::json!({"session_id":session_id}))
            }
            Self::Applications => ("application.list", serde_json::json!({})),
        };
        serde_json::json!({"protocol_version":API_VERSION,"request_id":request_id,"method":method,"params":params})
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    protocol_version: u64,
    request_id: String,
    method: String,
    params: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    base_name: String,
    template_name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    project_name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    session_id: String,
}
fn params<T: de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|_| Error("validation.invalid_request"))
}
fn decode(value: Value) -> Result<(Uuid, Command)> {
    let e: Envelope = params(value)?;
    let id = canonical_uuid(&e.request_id)?;
    if e.protocol_version != API_VERSION {
        return Err(Error("protocol.unsupported_version"));
    }
    if !e.params.is_object() || e.method.len() > 64 {
        return Err(Error("validation.invalid_request"));
    }
    let command = match e.method.as_str() {
        "project.list" | "template.list" | "session.list" | "session.deactivate"
        | "application.list" => {
            params::<Empty>(e.params)?;
            match e.method.as_str() {
                "project.list" => Command::Projects,
                "template.list" => Command::Templates,
                "session.list" => Command::Sessions,
                "session.deactivate" => Command::Deactivate,
                _ => Command::Applications,
            }
        }
        "project.create" => {
            let p: Create = params(e.params)?;
            Command::Create {
                base_name: p.base_name,
                template_name: p.template_name,
            }
        }
        "session.start" => {
            let p: Start = params(e.params)?;
            Command::Start {
                project_name: p.project_name,
            }
        }
        "session.switch" | "session.stop" => {
            let p: Target = params(e.params)?;
            let session_id = canonical_uuid(&p.session_id)?;
            if e.method == "session.switch" {
                Command::Switch { session_id }
            } else {
                Command::Stop { session_id }
            }
        }
        _ => return Err(Error("protocol.unknown_method")),
    };
    Ok((id, command))
}
pub(crate) fn canonical_uuid(text: &str) -> Result<Uuid> {
    let id = Uuid::parse_str(text).map_err(|_| Error("validation.invalid_request"))?;
    if id.to_string() != text {
        return Err(Error("validation.invalid_request"));
    }
    Ok(id)
}
pub struct Service {
    pub(crate) core: Core,
    resolver: ApplicationResolver,
}
impl Service {
    pub fn new(core: Core, resolver: ApplicationResolver) -> Self {
        Self { core, resolver }
    }
    pub fn handle(&self, line: &[u8]) -> Response {
        if line.len() > MAX_LINE_BYTES {
            return Response::failure(None, Error("protocol.message_too_large"));
        }
        let value = match strict_json(line) {
            Ok(v) => v,
            Err(e) => return Response::failure(None, e),
        };
        let id = value
            .get("request_id")
            .and_then(Value::as_str)
            .and_then(|s| canonical_uuid(s).ok());
        match decode(value) {
            Err(e) => Response::failure(id, e),
            Ok((id, command)) => match self.dispatch(command) {
                Ok(result) => Response {
                    protocol_version: API_VERSION,
                    request_id: Some(id),
                    kind: "response",
                    result: Some(result),
                    error: None,
                },
                Err(e) => Response::failure(Some(id), e),
            },
        }
    }
    fn dispatch(&self, command: Command) -> Result<Value> {
        let value = match command {
            Command::Projects => serde_json::json!({"projects":self.core.projects()?}),
            Command::Templates => {
                serde_json::json!({"templates":self.core.templates()?.into_iter().map(|name|serde_json::json!({"name":name})).collect::<Vec<_>>()})
            }
            Command::Create {
                base_name,
                template_name,
            } => {
                serde_json::json!({"project":self.core.create_project(&base_name,&template_name)?})
            }
            Command::Sessions => to_value(self.core.sessions()?)?,
            Command::Start { project_name } => to_value(self.core.start_result(&project_name)?)?,
            Command::Switch { session_id } => to_value(self.core.switch_result(session_id)?)?,
            Command::Stop { session_id } => to_value(self.core.stop_result(session_id)?)?,
            Command::Deactivate => to_value(self.core.deactivate()?)?,
            Command::Applications => to_value(self.core.applications(&self.resolver)?)?,
        };
        Ok(value)
    }
}
fn to_value(value: impl Serialize) -> Result<Value> {
    serde_json::to_value(value).map_err(|_| Error("internal.error"))
}

/// Reject duplicate object keys at every depth before decoding typed parameters.
pub(crate) fn strict_json(bytes: &[u8]) -> Result<Value> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = Unique::deserialize(&mut decoder).map_err(|_| Error("protocol.invalid_request"))?;
    decoder
        .end()
        .map_err(|_| Error("protocol.invalid_request"))?;
    Ok(value.0)
}
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        d.deserialize_any(UniqueVisitor)
    }
}
struct UniqueVisitor;
impl<'de> Visitor<'de> for UniqueVisitor {
    type Value = Unique;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value without duplicate keys")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
        Ok(Unique(Value::Bool(v)))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
        serde_json::Number::from_f64(v)
            .map(|n| Unique(Value::Number(n)))
            .ok_or_else(|| E::custom("invalid number"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
        Ok(Unique(Value::Null))
    }
    fn visit_none<E: de::Error>(self) -> std::result::Result<Unique, E> {
        Ok(Unique(Value::Null))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Unique, A::Error> {
        let mut values = Vec::new();
        while let Some(Unique(v)) = seq.next_element()? {
            values.push(v);
        }
        Ok(Unique(Value::Array(values)))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Unique, A::Error> {
        let mut keys = BTreeSet::new();
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom("duplicate key"));
            }
            let Unique(value) = map.next_value()?;
            values.insert(key, value);
        }
        Ok(Unique(Value::Object(values)))
    }
}
