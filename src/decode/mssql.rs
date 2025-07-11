use serde_json::Value as JsonValue;
use sqlx::mssql::MssqlValueRef;
use sqlx::value_ref::ValueRef;
use std::str;

pub fn to_json(value: MssqlValueRef<'_>) -> Result<JsonValue, sqlx::Error> {
    if value.is_null() {
        return Ok(JsonValue::Null);
    }

    // Attempt decoding based on common MSSQL types
    // You can extend this with other types as needed

    match value.type_info().name() {
        "int" => Ok(JsonValue::from(value.try_get::<i32>()?)),
        "bigint" => Ok(JsonValue::from(value.try_get::<i64>()?)),
        "bit" => Ok(JsonValue::from(value.try_get::<bool>()?)),
        "float" => Ok(JsonValue::from(value.try_get::<f64>()?)),
        "nvarchar" | "varchar" | "text" => {
            let s: &str = value.try_get()?;
            Ok(JsonValue::from(s.to_string()))
        }
        "uniqueidentifier" => {
            let guid: uuid::Uuid = value.try_get()?;
            Ok(JsonValue::from(guid.to_string()))
        }
        "datetime" | "datetime2" => {
            let dt: chrono::NaiveDateTime = value.try_get()?;
            Ok(JsonValue::from(dt.to_string()))
        }
        _ => {
            // Fallback: try to get as bytes and convert to string
            let bytes: &[u8] = value.try_get()?;
            let s = str::from_utf8(bytes).unwrap_or_default();
            Ok(JsonValue::from(s.to_string()))
        }
    }
}
