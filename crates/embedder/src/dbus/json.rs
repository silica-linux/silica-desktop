//! Conversion between JSON (the `silica/dbus` channel) and D-Bus values.
//!
//! Towards D-Bus the signature is needed: Dart sends the JSON values and the signature of the
//! arguments, and the conversion follows the signature (a number becomes `u`, `i` or `d`
//! depending on what the method expects). Inside a variant (`v`) there is no signature and the
//! type is deduced from the JSON. From D-Bus to JSON the conversion is lossless except for file
//! descriptors.

use serde_json::{Map, Number, Value as Json};
use zbus::zvariant::{Array, Dict, ObjectPath, Signature, Str, StructureBuilder, Value};

/// The values of `args` according to the argument signature (`""` for no arguments, `"us"`,
/// `"a{sv}"`…).
pub fn to_args(signature: &str, args: &[Json]) -> Result<Vec<Value<'static>>, String> {
    if signature.is_empty() {
        return if args.is_empty() {
            Ok(Vec::new())
        } else {
            Err(format!("{} arguments for the empty signature", args.len()))
        };
    }
    // Several complete types are read as the fields of a structure.
    let parsed = Signature::try_from(format!("({signature})").as_str())
        .map_err(|e| format!("invalid signature «{signature}»: {e}"))?;
    let Signature::Structure(fields) = &parsed else {
        return Err(format!("invalid signature «{signature}»"));
    };
    let fields: Vec<&Signature> = fields.iter().collect();
    if fields.len() != args.len() {
        return Err(format!(
            "signature «{signature}» wants {} arguments, {} arrived",
            fields.len(),
            args.len()
        ));
    }
    fields
        .into_iter()
        .zip(args)
        .map(|(sig, json)| to_value(sig, json))
        .collect()
}

/// A JSON value with the expected signature.
pub fn to_value(signature: &Signature, json: &Json) -> Result<Value<'static>, String> {
    let mismatch = || format!("{json} is not a value of type «{signature}»");
    let int = || json.as_i64().ok_or_else(mismatch);
    let uint = || json.as_u64().ok_or_else(mismatch);
    Ok(match signature {
        Signature::U8 => Value::U8(u8::try_from(uint()?).map_err(|_| mismatch())?),
        Signature::Bool => Value::Bool(json.as_bool().ok_or_else(mismatch)?),
        Signature::I16 => Value::I16(i16::try_from(int()?).map_err(|_| mismatch())?),
        Signature::U16 => Value::U16(u16::try_from(uint()?).map_err(|_| mismatch())?),
        Signature::I32 => Value::I32(i32::try_from(int()?).map_err(|_| mismatch())?),
        Signature::U32 => Value::U32(u32::try_from(uint()?).map_err(|_| mismatch())?),
        Signature::I64 => Value::I64(int()?),
        Signature::U64 => Value::U64(uint()?),
        Signature::F64 => Value::F64(json.as_f64().ok_or_else(mismatch)?),
        Signature::Str => Value::Str(Str::from(json.as_str().ok_or_else(mismatch)?.to_owned())),
        Signature::ObjectPath => {
            let path = json.as_str().ok_or_else(mismatch)?.to_owned();
            Value::ObjectPath(ObjectPath::try_from(path).map_err(|e| e.to_string())?)
        }
        Signature::Signature => {
            let text = json.as_str().ok_or_else(mismatch)?;
            Value::Signature(Signature::try_from(text).map_err(|e| e.to_string())?)
        }
        Signature::Variant => Value::Value(Box::new(infer(json)?)),
        Signature::Array(child) => {
            let items = json.as_array().ok_or_else(mismatch)?;
            let mut array = Array::new(child);
            for item in items {
                array
                    .append(to_value(child, item)?)
                    .map_err(|e| e.to_string())?;
            }
            Value::Array(array)
        }
        Signature::Dict { key, value } => {
            let object = json.as_object().ok_or_else(mismatch)?;
            let mut dict = Dict::new(key, value);
            for (k, v) in object {
                dict.append(dict_key(key, k)?, to_value(value, v)?)
                    .map_err(|e| e.to_string())?;
            }
            Value::Dict(dict)
        }
        Signature::Structure(fields) => {
            let items = json.as_array().ok_or_else(mismatch)?;
            let fields: Vec<&Signature> = fields.iter().collect();
            if fields.len() != items.len() {
                return Err(mismatch());
            }
            let mut builder = StructureBuilder::new();
            for (sig, item) in fields.into_iter().zip(items) {
                builder = builder.append_field(to_value(sig, item)?);
            }
            Value::Structure(builder.build().map_err(|e| e.to_string())?)
        }
        _ => return Err(format!("type «{signature}» not supported by the channel")),
    })
}

/// The keys of JSON objects are strings: for dictionaries with numeric keys they are read
/// as numbers.
fn dict_key(signature: &Signature, key: &str) -> Result<Value<'static>, String> {
    match signature {
        Signature::Str | Signature::ObjectPath | Signature::Signature => {
            to_value(signature, &Json::String(key.to_owned()))
        }
        Signature::Bool => to_value(signature, &Json::Bool(key == "true")),
        _ => {
            let number: Number = key
                .parse()
                .map_err(|_| format!("key «{key}» is not a number"))?;
            to_value(signature, &Json::Number(number))
        }
    }
}

/// The type of a value inside a variant, deduced from the JSON: integers `i` (or `x` if they
/// do not fit in 32 bits), decimals `d`, string lists `as`, other lists `av`, objects
/// `a{sv}`.
pub fn infer(json: &Json) -> Result<Value<'static>, String> {
    Ok(match json {
        Json::Null => return Err("null has no D-Bus type".to_owned()),
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => match i32::try_from(i) {
                Ok(small) => Value::I32(small),
                Err(_) => Value::I64(i),
            },
            (None, Some(u), _) => Value::U64(u),
            (_, _, Some(f)) => Value::F64(f),
            _ => return Err(format!("number {n} cannot be represented")),
        },
        Json::String(s) => Value::Str(Str::from(s.clone())),
        Json::Array(items) => {
            let signature = if items.iter().all(Json::is_string) {
                Signature::Str
            } else {
                Signature::Variant
            };
            to_value(&Signature::array(signature), json)?
        }
        Json::Object(_) => to_value(&Signature::dict(Signature::Str, Signature::Variant), json)?,
    })
}

/// A D-Bus value as JSON.
pub fn to_json(value: &Value<'_>) -> Json {
    match value {
        Value::U8(v) => Json::from(*v),
        Value::Bool(v) => Json::from(*v),
        Value::I16(v) => Json::from(*v),
        Value::U16(v) => Json::from(*v),
        Value::I32(v) => Json::from(*v),
        Value::U32(v) => Json::from(*v),
        Value::I64(v) => Json::from(*v),
        Value::U64(v) => Json::from(*v),
        Value::F64(v) => Number::from_f64(*v).map_or(Json::Null, Json::Number),
        Value::Str(v) => Json::from(v.as_str()),
        Value::Signature(v) => Json::from(v.to_string()),
        Value::ObjectPath(v) => Json::from(v.as_str()),
        Value::Value(inner) => to_json(inner),
        Value::Array(array) => Json::Array(array.inner().iter().map(to_json).collect()),
        Value::Dict(dict) => Json::Object(
            dict.iter()
                .map(|(k, v)| (key_string(k), to_json(v)))
                .collect::<Map<_, _>>(),
        ),
        Value::Structure(structure) => {
            Json::Array(structure.fields().iter().map(to_json).collect())
        }
        // File descriptors (and GVariant's Maybe) do not cross the channel.
        #[allow(unreachable_patterns)]
        _ => Json::Null,
    }
}

fn key_string(key: &Value<'_>) -> String {
    match to_json(key) {
        Json::String(s) => s,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roundtrip(signature: &str, value: Json) -> Json {
        let args = to_args(signature, std::slice::from_ref(&value)).unwrap();
        to_json(&args[0])
    }

    #[test]
    fn basic_types_follow_the_signature() {
        assert_eq!(roundtrip("u", json!(7)), json!(7));
        assert_eq!(roundtrip("y", json!(255)), json!(255));
        assert_eq!(roundtrip("d", json!(7)), json!(7.0));
        assert_eq!(roundtrip("o", json!("/a/b")), json!("/a/b"));
        assert_eq!(roundtrip("g", json!("a{sv}")), json!("a{sv}"));
        let args = to_args("us", &[json!(1), json!("x")]).unwrap();
        assert_eq!(args, [Value::U32(1), Value::from("x")]);
        assert!(to_args("", &[]).unwrap().is_empty());
    }

    #[test]
    fn containers_and_variants() {
        let dict = json!({"Volume": 0.5, "Muted": false, "Name": "Output", "Tags": ["a", "b"]});
        assert_eq!(roundtrip("a{sv}", dict.clone()), dict);
        assert_eq!(
            roundtrip("a(ou)", json!([["/x", 1], ["/y", 2]])),
            json!([["/x", 1], ["/y", 2]])
        );
        assert_eq!(
            roundtrip("a{us}", json!({"3": "three"})),
            json!({"3": "three"})
        );
    }

    #[test]
    fn bad_input_is_an_error_not_a_crash() {
        assert!(to_args("u", &[json!(-1)]).is_err());
        assert!(to_args("y", &[json!(256)]).is_err());
        assert!(to_args("o", &[json!("not a path")]).is_err());
        assert!(to_args("(ii)", &[json!([1])]).is_err());
        assert!(to_args("u", &[]).is_err());
        assert!(to_args("a{", &[json!(1)]).is_err());
        assert!(to_args("", &[json!(1)]).is_err());
        assert!(to_args("v", &[Json::Null]).is_err());
        assert!(to_args("h", &[json!(0)]).is_err());
    }
}
