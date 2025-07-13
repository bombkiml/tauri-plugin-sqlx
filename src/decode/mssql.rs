vuse serde_json::Value as JsonValue;
use sqlx::{value::ValueRef, TypeInfo};
use std::str;

use crate::Error;

pub fn to_json(value: sqlx::value::RawValue<'_>) -> Result<JsonValue, Error> {
    let type_info = value.type_info();

    match type_info.name() {
        // Common integer types
        "INT" | "BIGINT" | "SMALLINT" | "TINYINT" | "INTEGER" => {
            Ok(JsonValue::Number(value.as_i64()?.into()))
        }

        // MSSQL and Postgres float types
        "FLOAT" | "REAL" | "DOUBLE PRECISION" | "DECIMAL" | "NUMERIC" | "MONEY" | "SMALLMONEY" => {
            Ok(JsonValue::Number(
                serde_json::Number::from_f64(value.as_f64()?).ok_or_else(|| {
                    Error::UnsupportedDatatype("Invalid float conversion".to_string())
                })?,
            ))
        }

        // Boolean
        "BIT" | "BOOLEAN" => Ok(JsonValue::Bool(value.as_bool()?)),

        // Strings and text
        "CHAR" | "NCHAR" | "VARCHAR" | "NVARCHAR" | "TEXT" | "NTEXT" | "STRING" => {
            Ok(JsonValue::String(value.as_str()?.to_string()))
        }

        // Date and time types
        "DATE" | "DATETIME" | "DATETIME2" | "SMALLDATETIME" | "TIMESTAMP" | "TIME" => {
            Ok(JsonValue::String(value.as_str()?.to_string()))
        }

        // UUID
        "UNIQUEIDENTIFIER" | "UUID" => Ok(JsonValue::String(value.as_str()?.to_string())),

        // Fallback for unknown or unhandled types
        other => Err(Error::UnsupportedDatatype(other.to_string())),
    }
}
