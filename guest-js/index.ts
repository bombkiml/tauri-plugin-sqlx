import { invoke } from "@tauri-apps/api/core"

export interface QueryResult {
  /** The number of rows affected by the query. */
  rowsAffected: number;
  /**
   * The last inserted `id`.
   *
   * This value is always `0` when using the Postgres driver. If the
   * last inserted id is required on Postgres, the `select` function
   * must be used, with a `RETURNING` clause
   * (`INSERT INTO todos (title) VALUES ($1) RETURNING id`).
   */
  lastInsertId: number;
}

/**
 * **Database**
 *
 * The `Database` class serves as the primary interface for
 * communicating with the rust side of the sql plugin.
 */
export default class Database {
  path: string;
  constructor(path: string) {
    this.path = path;
  }

  /**
   * **load**
   *
   * A static initializer which connects to the underlying database and
   * returns a `Database` instance once a connection to the database is established.
   *
   * # Sqlite
   *
   * The path is relative to `tauri::api::path::BaseDirectory::App` and must start with `sqlite:`.
   *
   * @example
   * ```ts
   * const db = await Database.load("sqlite:test.db");
   * ```
   */
  static async load(path: string): Promise<Database> {
    const _path = await invoke<string>("plugin:sql|load", {
      db: path,
    });

    return new Database(_path);
  }


  public async load2(): Promise<Database> {
    const _path:string = await invoke('plugin:sql|load', {
      db: "mssql://dbuser:password@localhost:1433/mssql_beech_test"
    });

    // const _path = await invoke<string>("plugin:sql|load", {
    //   db: path,
    // });

    return new Database(_path);
  }

  /**
   * **get**
   *
   * A static initializer which synchronously returns an instance of
   * the Database class while deferring the actual database connection
   * until the first invocation or selection on the database.
   *
   * # Sqlite
   *
   * The path is relative to `tauri::api::path::BaseDirectory::App` and must start with `sqlite:`.
   *
   * @example
   * ```ts
   * const db = Database.get("sqlite:test.db");
   * ```
   */
  static get(path: string): Database {
    return new Database(path);
  }

  /**
   * **execute**
   *
   * Passes a SQL expression to the database for execution.
   *
   * @example
   * ```ts
   * const result = await db.execute(
   *    "UPDATE todos SET title = $1, completed = $2 WHERE id = $3",
   *    [ todos.title, todos.status, todos.id ]
   * );
   * ```
   */
  async execute(query: string, bindValues?: unknown[]): Promise<QueryResult> {
    const [rowsAffected, lastInsertId] = await invoke<[number, number]>(
      "plugin:sql|execute",
      {
        db: this.path,
        query,
        values: bindValues ?? [],
      }
    );

    return {
      lastInsertId,
      rowsAffected,
    };
  }

  /**
   * **select**
   *
   * Passes in a SELECT query to the database for execution.
   *
   * @example
   * ```ts
   * const result = await db.select(
   *    "SELECT * from todos WHERE id = $1", id
   * );
   * ```
   */
  async select<T>(query: string, bindValues?: unknown[]): Promise<T> {
    const result = await invoke<T>("plugin:sql|select", {
      db: this.path,
      query,
      values: bindValues ?? [],
    });

    return result;
  }

  /**
   * **close**
   *
   * Closes the database connection pool.
   *
   * @example
   * ```ts
   * const success = await db.close()
   * ```
   * @param db - Optionally state the name of a database if you are managing more than one. Otherwise, all database pools will be in scope.
   */
  async close(db?: string): Promise<boolean> {
    const success = await invoke<boolean>("plugin:sql|close", {
      db,
    });
    return success;
  }
}
