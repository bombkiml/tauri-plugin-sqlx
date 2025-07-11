// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(feature = "sqlite")]
use std::fs::create_dir_all;

use indexmap::IndexMap;
use serde_json::Value as JsonValue;

#[cfg(any(feature = "sqlite", feature = "mysql", feature = "postgres", feature = "mssql"))]
use sqlx::{migrate::MigrateDatabase, Column, Executor, Pool, Row};

use tauri::{AppHandle, Runtime};

#[cfg(feature = "mysql")]
use sqlx::MySql;
#[cfg(feature = "postgres")]
use sqlx::Postgres;
#[cfg(feature = "sqlite")]
use sqlx::Sqlite;
#[cfg(feature = "mssql")]
use sqlx::Mssql;

use crate::LastInsertId;

pub enum DbPool {
    #[cfg(feature = "sqlite")]
    Sqlite(Pool<Sqlite>),
    #[cfg(feature = "mysql")]
    MySql(Pool<MySql>),
    #[cfg(feature = "postgres")]
    Postgres(Pool<Postgres>),
    #[cfg(feature = "mssql")]
    Mssql(Pool<Mssql>),
    #[cfg(not(any(feature = "sqlite", feature = "mysql", feature = "postgres", feature = "mssql")))]
    None,
}

impl DbPool {
    pub(crate) async fn connect<R: Runtime>(
        conn_url: &str,
        _app: &AppHandle<R>,
    ) -> Result<Self, crate::Error> {
        let scheme = conn_url
            .split_once(':')
            .ok_or_else(|| crate::Error::InvalidDbUrl(conn_url.to_string()))?
            .0;

        match scheme {
            #[cfg(feature = "sqlite")]
            "sqlite" => {
                let app_path = _app
                    .path()
                    .app_config_dir()
                    .expect("No App config path was found!");

                create_dir_all(&app_path).expect("Couldn't create app config dir");

                let conn_url = &path_mapper(app_path, conn_url);

                if !Sqlite::database_exists(conn_url).await.unwrap_or(false) {
                    Sqlite::create_database(conn_url).await?;
                }
                Ok(Self::Sqlite(Pool::<Sqlite>::connect(conn_url).await?))
            }
            #[cfg(feature = "mysql")]
            "mysql" => {
                if !MySql::database_exists(conn_url).await.unwrap_or(false) {
                    MySql::create_database(conn_url).await?;
                }
                Ok(Self::MySql(Pool::<MySql>::connect(conn_url).await?))
            }
            #[cfg(feature = "postgres")]
            "postgres" => {
                if !Postgres::database_exists(conn_url).await.unwrap_or(false) {
                    Postgres::create_database(conn_url).await?;
                }
                Ok(Self::Postgres(Pool::<Postgres>::connect(conn_url).await?))
            }
            #[cfg(feature = "mssql")]
            "mssql" => {
                // sqlx 0.5 doesn't have create_database or database_exists for MSSQL
                // So just connect, expect DB exists.
                Ok(Self::Mssql(Pool::<Mssql>::connect(conn_url).await?))
            }
            #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql")))]
            _ => Err(crate::Error::InvalidDbUrl(format!(
                "{conn_url} - No database driver enabled!"
            ))),
            #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql"))]
            _ => Err(crate::Error::InvalidDbUrl(conn_url.to_string())),
        }
    }

    pub(crate) async fn migrate(
        &self,
        _migrator: &sqlx::migrate::Migrator,
    ) -> Result<(), crate::Error> {
        match self {
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(pool) => _migrator.run(pool).await?,
            #[cfg(feature = "mysql")]
            DbPool::MySql(pool) => _migrator.run(pool).await?,
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => _migrator.run(pool).await?,
            #[cfg(feature = "mssql")]
            DbPool::Mssql(pool) => _migrator.run(pool).await?,
            #[cfg(not(any(feature = "sqlite", feature = "mysql", feature = "postgres", feature = "mssql")))]
            DbPool::None => (),
        }
        Ok(())
    }

    pub(crate) async fn close(&self) {
        match self {
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(pool) => pool.close().await,
            #[cfg(feature = "mysql")]
            DbPool::MySql(pool) => pool.close().await,
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => pool.close().await,
            #[cfg(feature = "mssql")]
            DbPool::Mssql(pool) => pool.close().await,
            #[cfg(not(any(feature = "sqlite", feature = "mysql", feature = "postgres", feature = "mssql")))]
            DbPool::None => (),
        }
    }

    pub(crate) async fn execute(
        &self,
        _query: String,
        _values: Vec<JsonValue>,
    ) -> Result<(u64, LastInsertId), crate::Error> {
        Ok(match self {
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap());
                    } else if let Some(number) = value.as_f64() {
                        query = query.bind(number);
                    } else {
                        query = query.bind(value);
                    }
                }
                let result = pool.execute(query).await?;
                (
                    result.rows_affected(),
                    LastInsertId::Sqlite(result.last_insert_rowid()),
                )
            }
            #[cfg(feature = "mysql")]
            DbPool::MySql(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap());
                    } else if let Some(number) = value.as_f64() {
                        query = query.bind(number);
                    } else {
                        query = query.bind(value);
                    }
                }
                let result = pool.execute(query).await?;
                (
                    result.rows_affected(),
                    LastInsertId::MySql(result.last_insert_id()),
                )
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap());
                    } else if let Some(number) = value.as_f64() {
                        query = query.bind(number);
                    } else {
                        query = query.bind(value);
                    }
                }
                let result = pool.execute(query).await?;
                (result.rows_affected(), LastInsertId::Postgres(()))
            }
            #[cfg(feature = "mssql")]
            DbPool::Mssql(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap());
                    } else if let Some(number) = value.as_f64() {
                        query = query.bind(number);
                    } else {
                        query = query.bind(value);
                    }
                }
                let result = pool.execute(query).await?;
                // MSSQL doesn't provide last_insert_id
                (result.rows_affected(), LastInsertId::None)
            }
            #[cfg(not(any(feature = "sqlite", feature = "mysql", feature = "postgres", feature = "mssql")))]
            DbPool::None => (0, LastInsertId::None),
        })
    }

    pub(crate) async fn select(
        &self,
        _query: String,
        _values: Vec<JsonValue>,
    ) -> Result<Vec<IndexMap<String, JsonValue>>, crate::Error> {
        Ok(match self {
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values.iter() {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap());
                    } else if let Some(number) = value.as_f64() {
                        query = query.bind(number);
                    } else {
                        query = query.bind(value);
                    }
                }
                let rows = pool.fetch_all(query).await?;
                let mut values = Vec::new();
                for row in rows {
                    let mut value = IndexMap::default();
                    for (i, column) in row.columns().iter().enumerate() {
                        let v = row.try_get_raw(i)?;
                        let v = crate::decode::sqlite::to_json(v)?;
                        value.insert(column.name().to_string(), v);
                    }
                    values.push(value);
                }
                values
            }
            #[cfg(feature = "mysql")]
            DbPool::MySql(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values.iter() {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap());
                    } else if let Some(number) = value.as_f64() {
                        query = query.bind(number);
                    } else {
                        query = query.bind(value);
                    }
                }
                let rows = pool.fetch_all(query).await?;
                let mut values = Vec::new();
                for row in rows {
                    let mut value = IndexMap::default();
                    for (i, column) in row.columns().iter().enumerate() {
                        let v = row.try_get_raw(i)?;
                        let v = crate::decode::mysql::to_json(v)?;
                        value.insert(column.name().to_string(), v);
                    }
                    values.push(value);
                }
                values
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values.iter() {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap());
                    } else if let Some(number) = value.as_f64() {
                        query = query.bind(number);
                    } else {
                        query = query.bind(value);
                    }
                }
                let rows = pool.fetch_all(query).await?;
                let mut values = Vec::new();
                for row in rows {
                    let mut value = IndexMap::default();
                    for (i, column) in row.columns().iter().enumerate() {
                        let v = row.try_get_raw(i)?;
                        let v = crate::decode::postgres::to_json(v)?;
                        value.insert(column.name().to_string(), v);
                    }
                    values.push(value);
                }
                values
            }
            #[cfg(feature = "mssql")]
            DbPool::Mssql(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values.iter() {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap());
                    } else if let Some(number) = value.as_f64() {
                        query = query.bind(number);
                    } else {
                        query = query.bind(value);
                    }
                }
                let rows = pool.fetch_all(query).await?;
                let mut values = Vec::new();
                for row in rows {
                    let mut value = IndexMap::default();
                    for (i, column) in row.columns().iter().enumerate() {
                        let v = row.try_get_raw(i)?;
                        let v = crate::decode::mssql::to_json(v)?;
                        value.insert(column.name().to_string(), v);
                    }
                    values.push(value);
                }
                values
            }
            #[cfg(not(any(feature = "sqlite", feature = "mysql", feature = "postgres", feature = "mssql")))]
            DbPool::None => Vec::new(),
        })
    }
}

#[cfg(feature = "sqlite")]
fn path_mapper(mut app_path: std::path::PathBuf, connection_string: &str) -> String {
    app_path.push(
        connection_string
            .split_once(':')
            .expect("Couldn't parse the connection string for DB!")
            .1,
    );

    format!(
        "sqlite:{}",
        app_path
            .to_str()
            .expect("Problem creating fully qualified path to Database file!")
    )
}
