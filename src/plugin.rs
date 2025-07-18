use futures_core::future::BoxFuture;
use serde::{ser::Serializer, Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{
    error::BoxDynError,
    migrate::{
        MigrateDatabase, Migration as SqlxMigration, MigrationSource, MigrationType, Migrator,
    },
    Column, Pool, Row,
};
use tauri::{
    command,
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, RunEvent, Runtime, State,
};
use tokio::sync::Mutex;

use std::collections::HashMap;

#[cfg(feature = "sqlite")]
use std::{fs::create_dir_all, path::PathBuf};

// ==== Database Driver Selection ====

#[cfg(feature = "sqlite")]
type Db = sqlx::sqlite::Sqlite;
#[cfg(feature = "mysql")]
type Db = sqlx::mysql::MySql;
#[cfg(feature = "postgres")]
type Db = sqlx::postgres::Postgres;
#[cfg(feature = "mssql")]
type Db = sqlx::mssql::Mssql;

#[cfg(feature = "sqlite")]
type LastInsertId = i64;
#[cfg(any(feature = "mysql", feature = "mssql"))]
type LastInsertId = u64;
#[cfg(feature = "postgres")]
type LastInsertId = u64; // Always returns 0 in postgres

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("database {0} not loaded")]
    DatabaseNotLoaded(String),
    #[error("unsupported datatype: {0}")]
    UnsupportedDatatype(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "sqlite")]
fn app_path<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path().app_config_dir().expect("No App path was found!")
}

#[cfg(feature = "sqlite")]
fn path_mapper(mut app_path: PathBuf, connection_string: &str) -> String {
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

#[derive(Default)]
struct DbInstances(Mutex<HashMap<String, Pool<Db>>>);

struct Migrations(Mutex<HashMap<String, MigrationList>>);

#[derive(Default, Clone, Deserialize)]
pub struct PluginConfig {
    #[serde(default)]
    preload: Vec<String>,
}

#[derive(Debug)]
pub enum MigrationKind {
    Up,
    Down,
}

impl From<MigrationKind> for MigrationType {
    fn from(kind: MigrationKind) -> Self {
        match kind {
            MigrationKind::Up => Self::ReversibleUp,
            MigrationKind::Down => Self::ReversibleDown,
        }
    }
}

#[derive(Debug)]
pub struct Migration {
    pub version: i64,
    pub description: &'static str,
    pub sql: &'static str,
    pub kind: MigrationKind,
}

#[derive(Debug)]
struct MigrationList(Vec<Migration>);

impl MigrationSource<'static> for MigrationList {
    fn resolve(self) -> BoxFuture<'static, std::result::Result<Vec<SqlxMigration>, BoxDynError>> {
        Box::pin(async move {
            let mut migrations = Vec::new();
            for migration in self.0 {
                if matches!(migration.kind, MigrationKind::Up) {
                    migrations.push(SqlxMigration::new(
                        migration.version,
                        migration.description.into(),
                        migration.kind.into(),
                        migration.sql.into(),
                    ));
                }
            }
            Ok(migrations)
        })
    }
}

#[command]
async fn load<R: Runtime>(
    #[allow(unused_variables)] app: AppHandle<R>,
    db_instances: State<'_, DbInstances>,
    migrations: State<'_, Migrations>,
    db: String,
) -> Result<String> {
    #[cfg(feature = "sqlite")]
    let fqdb = path_mapper(app_path(&app), &db);
    #[cfg(not(feature = "sqlite"))]
    let fqdb = db.clone();

    #[cfg(feature = "sqlite")]
    create_dir_all(app_path(&app)).expect("Problem creating App directory!");

    if !Db::database_exists(&fqdb).await.unwrap_or(false) {
        Db::create_database(&fqdb).await?;
    }

    let pool = Pool::connect(&fqdb).await?;

    if let Some(migrations) = migrations.0.lock().await.remove(&db) {
        let migrator = Migrator::new(migrations).await?;
        migrator.run(&pool).await?;
    }

    db_instances.0.lock().await.insert(db.clone(), pool);
    Ok(db)
}

#[command]
async fn close(db_instances: State<'_, DbInstances>, db: Option<String>) -> Result<bool> {
    let mut instances = db_instances.0.lock().await;

    let pools = if let Some(db) = db {
        vec![db]
    } else {
        instances.keys().cloned().collect()
    };

    for pool in pools {
        let db = instances
            .get_mut(&pool)
            .ok_or(Error::DatabaseNotLoaded(pool))?;
        db.close().await;
    }

    Ok(true)
}

#[command]
async fn execute(
    db_instances: State<'_, DbInstances>,
    db: String,
    query: String,
    values: Vec<JsonValue>,
) -> Result<(u64, LastInsertId)> {
    let mut instances = db_instances.0.lock().await;
    let db = instances.get_mut(&db).ok_or(Error::DatabaseNotLoaded(db))?;

    let mut query = sqlx::query(&query);
    for value in values {
        if value.is_null() {
            query = query.bind(None::<JsonValue>);
        } else if value.is_string() {
            query = query.bind(value.as_str().unwrap().to_owned());
        } else {
            query = query.bind(value);
        }
    }

    let result = query.execute(&*db).await?;

    #[cfg(feature = "sqlite")]
    let r = Ok((result.rows_affected(), result.last_insert_rowid()));
    #[cfg(feature = "mysql")]
    let r = Ok((result.rows_affected(), result.last_insert_id()));
    #[cfg(feature = "postgres")]
    let r = Ok((result.rows_affected(), 0));
    #[cfg(feature = "mssql")]
    let r = Ok((result.rows_affected(), result.last_insert_id()));

    r
}

#[command]
async fn select(
    db_instances: State<'_, DbInstances>,
    db: String,
    query: String,
    values: Vec<JsonValue>,
) -> Result<Vec<HashMap<String, JsonValue>>> {
    let mut instances = db_instances.0.lock().await;
    let db = instances.get_mut(&db).ok_or(Error::DatabaseNotLoaded(db))?;

    let mut query = sqlx::query(&query);
    for value in values {
        if value.is_null() {
            query = query.bind(None::<JsonValue>);
        } else if value.is_string() {
            query = query.bind(value.as_str().unwrap().to_owned());
        } else {
            query = query.bind(value);
        }
    }

    let rows = query.fetch_all(&*db).await?;
    let mut values = Vec::new();

    for row in rows {
        let mut value = HashMap::default();
        for (i, column) in row.columns().iter().enumerate() {
            let v = row.try_get_raw(i)?;
            let v = crate::decode::to_json(v)?;
            value.insert(column.name().to_string(), v);
        }
        values.push(value);
    }

    Ok(values)
}

/// Tauri SQL plugin builder.
#[derive(Default)]
pub struct Builder {
  migrations: Option<HashMap<String, MigrationList>>,
}

impl Builder {
  /// Add migrations to a database.
  #[must_use]
  pub fn add_migrations(mut self, db_url: &str, migrations: Vec<Migration>) -> Self {
      self.migrations
          .get_or_insert(Default::default())
          .insert(db_url.to_string(), MigrationList(migrations));
      self
  }

  pub fn build<R: Runtime>(mut self) -> TauriPlugin<R, Option<PluginConfig>> {
      PluginBuilder::<R, Option<PluginConfig>>::new("sql")
          .js_init_script(include_str!("api-iife.js").to_string())
          .invoke_handler(tauri::generate_handler![load, execute, select, close])
          .setup(|app, api| {
              let config = api.config().clone().unwrap_or_default();

              #[cfg(feature = "sqlite")]
              create_dir_all(app_path(app)).expect("problems creating App directory!");

              tauri::async_runtime::block_on(async move {
                  let instances = DbInstances::default();
                  let mut lock = instances.0.lock().await;
                  for db in config.preload {
                      #[cfg(feature = "sqlite")]
                      let fqdb = path_mapper(app_path(app), &db);
                      #[cfg(not(feature = "sqlite"))]
                      let fqdb = db.clone();

                      if !Db::database_exists(&fqdb).await.unwrap_or(false) {
                          Db::create_database(&fqdb).await?;
                      }
                      let pool = Pool::connect(&fqdb).await?;

                      if let Some(migrations) = self.migrations.as_mut().unwrap().remove(&db) {
                          let migrator = Migrator::new(migrations).await?;
                          migrator.run(&pool).await?;
                      }
                      lock.insert(db, pool);
                  }
                  drop(lock);

                  app.manage(instances);
                  app.manage(Migrations(Mutex::new(
                      self.migrations.take().unwrap_or_default(),
                  )));

                  Ok(())
              })
          })
          .on_event(|app, event| {
              if let RunEvent::Exit = event {
                  tauri::async_runtime::block_on(async move {
                      let instances = &*app.state::<DbInstances>();
                      let instances = instances.0.lock().await;
                      for value in instances.values() {
                          value.close().await;
                      }
                  });
              }
          })
          .build()
  }
}