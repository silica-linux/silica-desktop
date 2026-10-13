//! Conversione fra JSON (il canale `silica/dbus`) e valori D-Bus.
//!
//! Verso D-Bus serve la firma: Dart manda i valori JSON e la firma degli argomenti, e la
//! conversione segue la firma (un numero diventa `u`, `i` o `d` a seconda di cosa si
//! aspetta il metodo). Dentro un variante (`v`) la firma non c'è e il tipo si deduce dal
//! JSON. Da D-Bus a JSON la conversione è senza perdite salvo i descrittori di file.

use serde_json::{Map, Number, Value as Json};
use zbus::zvariant::{Array, Dict, ObjectPath, Signature, Str, StructureBuilder, Value};

/// I valori di `args` secondo la firma degli argomenti (`""` per nessun argomento, `"us"`,
/// `"a{sv}"`…).
pub fn to_args(signature: &str, args: &[Json]) -> Result<Vec<Value<'static>>, String> {
    if signature.is_empty() {
        return if args.is_empty() {
            Ok(Vec::new())
        } else {
            Err(format!("{} argomenti per la firma vuota", args.len()))
        };
    }
    // Più tipi completi si leggono come i campi di una struttura.
    let parsed = Signature::try_from(format!("({signature})").as_str())
        .map_err(|e| format!("firma «{signature}» non valida: {e}"))?;
    let Signature::Structure(fields) = &parsed else {
        return Err(format!("firma «{signature}» non valida"));
    };
    let fields: Vec<&Signature> = fields.iter().collect();
    if fields.len() != args.len() {
        return Err(format!(
            "la firma «{signature}» vuole {} argomenti, ne sono arrivati {}",
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

/// Un valore JSON con la firma attesa.
pub fn to_value(signature: &Signature, json: &Json) -> Result<Value<'static>, String> {
    let mismatch = || format!("{json} non è un valore di tipo «{signature}»");
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
        _ => return Err(format!("tipo «{signature}» non supportato dal canale")),
    })
}

/// Le chiavi degli oggetti JSON sono stringhe: per i dizionari con chiavi numeriche si
/// leggono come numeri.
fn dict_key(signature: &Signature, key: &str) -> Result<Value<'static>, String> {
    match signature {
        Signature::Str | Signature::ObjectPath | Signature::Signature => {
            to_value(signature, &Json::String(key.to_owned()))
        }
        Signature::Bool => to_value(signature, &Json::Bool(key == "true")),
        _ => {
            let number: Number = key
                .parse()
                .map_err(|_| format!("chiave «{key}» non è un numero"))?;
            to_value(signature, &Json::Number(number))
        }
    }
}

/// Il tipo di un valore dentro un variante, dedotto dal JSON: interi `i` (o `x` se non
/// stanno in 32 bit), decimali `d`, liste di stringhe `as`, altre liste `av`, oggetti
/// `a{sv}`.
pub fn infer(json: &Json) -> Result<Value<'static>, String> {
    Ok(match json {
        Json::Null => return Err("null non ha un tipo D-Bus".to_owned()),
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => match i32::try_from(i) {
                Ok(small) => Value::I32(small),
                Err(_) => Value::I64(i),
            },
            (None, Some(u), _) => Value::U64(u),
            (_, _, Some(f)) => Value::F64(f),
            _ => return Err(format!("numero {n} non rappresentabile")),
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

/// Un valore D-Bus in JSON.
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
        // Descrittori di file (e i Maybe di GVariant) non attraversano il canale.
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
        let dict = json!({"Volume": 0.5, "Muted": false, "Name": "Uscita", "Tags": ["a", "b"]});
        assert_eq!(roundtrip("a{sv}", dict.clone()), dict);
        assert_eq!(
            roundtrip("a(ou)", json!([["/x", 1], ["/y", 2]])),
            json!([["/x", 1], ["/y", 2]])
        );
        assert_eq!(roundtrip("a{us}", json!({"3": "tre"})), json!({"3": "tre"}));
    }

    #[test]
    fn bad_input_is_an_error_not_a_crash() {
        assert!(to_args("u", &[json!(-1)]).is_err());
        assert!(to_args("y", &[json!(256)]).is_err());
        assert!(to_args("o", &[json!("non un percorso")]).is_err());
        assert!(to_args("(ii)", &[json!([1])]).is_err());
        assert!(to_args("u", &[]).is_err());
        assert!(to_args("a{", &[json!(1)]).is_err());
        assert!(to_args("", &[json!(1)]).is_err());
        assert!(to_args("v", &[Json::Null]).is_err());
        assert!(to_args("h", &[json!(0)]).is_err());
    }
}
