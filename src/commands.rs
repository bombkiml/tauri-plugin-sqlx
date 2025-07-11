use indexmap::IndexMap;
use serde_json::Value as JsonValue;
use sqlx::migrate::Migrator;
use tauri::{command, AppHandle, Runtime, State};

use crate::{DbInstances, DbPool, Error, LastInsertId, Migrations};

#[command]
pub(crate) async fn load<R: Runtime>(
    app: AppHandle<R>,
    db_instances: State<'_, DbInstances>,
    migrations: State<'_, Migrations>,
    db: String,
) -> Result<String, crate::Error> {
    let pool = DbPool::connect(&db, &app).await?;

    if let Some(migration_list) = migrations.0.lock().await.remove(&db) {
        let migrator = Migrator::new(migration_list).await?;
        pool.migrate(&migrator).await?;
    }

    db_instances.0.write().await.insert(db.clone(), pool);

    Ok(db)
}

#[command]
pub(crate) async fn close(
    db_instances: State<'_, DbInstances>,
    db: Option<String>,
) -> Result<bool, crate::Error> {
    let instances = db_instances.0.read().await;

    let pools = if let Some(db) = db {
        vec![db]
    } else {
        instances.keys().cloned().collect()
    };

    for pool_name in pools {
        let db = instances.get(&pool_name).ok_or(Error::DatabaseNotLoaded(pool_name.clone()))?;
        db.close().await;
    }

    Ok(true)
}

#[command]
pub(crate) async fn execute(
    db_instances: State<'_, DbInstances>,
    db: String,
    query: String,
    values: Vec<JsonValue>,
) -> Result<(u64, LastInsertId), crate::Error> {
    let instances = db_instances.0.read().await;

    let db = instances.get(&db).ok_or(Error::DatabaseNotLoaded(db.clone()))?;
    db.execute(query, values).await
}

#[command]
pub(crate) async fn select(
    db_instances: State<'_, DbInstances>,
    db: String,
    query: String,
    values: Vec<JsonValue>,
) -> Result<Vec<IndexMap<String, JsonValue>>, crate::Error> {
    let instances = db_instances.0.read().await;

    let db = instances.get(&db).ok_or(Error::DatabaseNotLoaded(db.clone()))?;
    db.select(query, values).await
}
