pub trait Database: Send {
    fn ensure_tracking_table(&mut self, table_name: &str) -> Result<(), String>;
    fn is_seed_applied(&mut self, table_name: &str, seed_set: &str) -> Result<bool, String>;
    fn mark_seed_applied(&mut self, table_name: &str, seed_set: &str) -> Result<(), String>;
    fn remove_seed_mark(&mut self, table_name: &str, seed_set: &str) -> Result<(), String>;
    fn insert_row(
        &mut self,
        table: &str,
        columns: &[String],
        values: &[String],
        auto_id_column: Option<&str>,
    ) -> Result<Option<i64>, String>;
    fn row_exists(
        &mut self,
        table: &str,
        unique_columns: &[String],
        unique_values: &[String],
    ) -> Result<bool, String>;
    fn delete_rows(&mut self, table: &str) -> Result<u64, String>;
    fn begin_transaction(&mut self) -> Result<(), String>;
    fn commit_transaction(&mut self) -> Result<(), String>;
    fn rollback_transaction(&mut self) -> Result<(), String>;
    fn create_database(&mut self, name: &str) -> Result<(), String>;
    fn create_schema(&mut self, name: &str) -> Result<(), String>;
    fn object_exists(&mut self, obj_type: &str, name: &str) -> Result<bool, String>;
    fn driver_name(&self) -> &str;
}

#[cfg(feature = "sqlite")]
pub struct SqliteDb {
    pub(crate) conn: rusqlite::Connection,
    in_transaction: bool,
}

#[cfg(feature = "sqlite")]
impl SqliteDb {
    pub fn connect(url: &str) -> Result<Self, String> {
        let conn = if url == ":memory:" {
            rusqlite::Connection::open_in_memory()
        } else {
            rusqlite::Connection::open(url)
        }
        .map_err(|e| format!("opening sqlite database '{}': {}", url, e))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| format!("setting sqlite pragmas: {}", e))?;
        Ok(Self {
            conn,
            in_transaction: false,
        })
    }
}

#[cfg(feature = "sqlite")]
impl Database for SqliteDb {
    fn ensure_tracking_table(&mut self, table_name: &str) -> Result<(), String> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS \"{}\" (
                seed_set TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            sanitize_identifier(table_name)
        );
        self.conn
            .execute(&sql, [])
            .map_err(|e| format!("creating tracking table: {}", e))?;
        Ok(())
    }

    fn is_seed_applied(&mut self, table_name: &str, seed_set: &str) -> Result<bool, String> {
        let sql = format!(
            "SELECT COUNT(*) FROM \"{}\" WHERE seed_set = ?1",
            sanitize_identifier(table_name)
        );
        let count: i64 = self
            .conn
            .query_row(&sql, [seed_set], |row| row.get(0))
            .map_err(|e| format!("checking seed status: {}", e))?;
        Ok(count > 0)
    }

    fn mark_seed_applied(&mut self, table_name: &str, seed_set: &str) -> Result<(), String> {
        let sql = format!(
            "INSERT OR IGNORE INTO \"{}\" (seed_set) VALUES (?1)",
            sanitize_identifier(table_name)
        );
        self.conn
            .execute(&sql, [seed_set])
            .map_err(|e| format!("marking seed applied: {}", e))?;
        Ok(())
    }

    fn remove_seed_mark(&mut self, table_name: &str, seed_set: &str) -> Result<(), String> {
        let sql = format!(
            "DELETE FROM \"{}\" WHERE seed_set = ?1",
            sanitize_identifier(table_name)
        );
        self.conn
            .execute(&sql, [seed_set])
            .map_err(|e| format!("removing seed mark: {}", e))?;
        Ok(())
    }

    fn insert_row(
        &mut self,
        table: &str,
        columns: &[String],
        values: &[String],
        _auto_id_column: Option<&str>,
    ) -> Result<Option<i64>, String> {
        let col_list: Vec<String> = columns
            .iter()
            .map(|c| format!("\"{}\"", sanitize_identifier(c)))
            .collect();
        let placeholders: Vec<String> = (1..=values.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "INSERT INTO \"{}\" ({}) VALUES ({})",
            sanitize_identifier(table),
            col_list.join(", "),
            placeholders.join(", ")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = values
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();
        self.conn
            .execute(&sql, params.as_slice())
            .map_err(|e| format!("inserting row into '{}': {}", table, e))?;
        Ok(Some(self.conn.last_insert_rowid()))
    }

    fn row_exists(
        &mut self,
        table: &str,
        unique_columns: &[String],
        unique_values: &[String],
    ) -> Result<bool, String> {
        if unique_columns.is_empty() {
            return Ok(false);
        }
        let conditions: Vec<String> = unique_columns
            .iter()
            .enumerate()
            .map(|(i, c)| format!("\"{}\" = ?{}", sanitize_identifier(c), i + 1))
            .collect();
        let sql = format!(
            "SELECT COUNT(*) FROM \"{}\" WHERE {}",
            sanitize_identifier(table),
            conditions.join(" AND ")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = unique_values
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();
        let count: i64 = self
            .conn
            .query_row(&sql, params.as_slice(), |row| row.get(0))
            .map_err(|e| format!("checking row existence in '{}': {}", table, e))?;
        Ok(count > 0)
    }

    fn delete_rows(&mut self, table: &str) -> Result<u64, String> {
        let sql = format!("DELETE FROM \"{}\"", sanitize_identifier(table));
        let count = self
            .conn
            .execute(&sql, [])
            .map_err(|e| format!("deleting rows from '{}': {}", table, e))?;
        Ok(count as u64)
    }

    fn begin_transaction(&mut self) -> Result<(), String> {
        self.conn
            .execute("BEGIN", [])
            .map_err(|e| format!("beginning transaction: {}", e))?;
        self.in_transaction = true;
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<(), String> {
        if self.in_transaction {
            self.conn
                .execute("COMMIT", [])
                .map_err(|e| format!("committing transaction: {}", e))?;
            self.in_transaction = false;
        }
        Ok(())
    }

    fn rollback_transaction(&mut self) -> Result<(), String> {
        if self.in_transaction {
            self.conn
                .execute("ROLLBACK", [])
                .map_err(|e| format!("rolling back transaction: {}", e))?;
            self.in_transaction = false;
        }
        Ok(())
    }

    fn create_database(&mut self, _name: &str) -> Result<(), String> {
        Err("sqlite does not support CREATE DATABASE (each file is a database)".into())
    }

    fn create_schema(&mut self, _name: &str) -> Result<(), String> {
        Err("sqlite does not support schemas".into())
    }

    fn object_exists(&mut self, obj_type: &str, name: &str) -> Result<bool, String> {
        match obj_type {
            "table" => {
                let count: i64 = self
                    .conn
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                        [name],
                        |row| row.get(0),
                    )
                    .map_err(|e| format!("checking table existence: {}", e))?;
                Ok(count > 0)
            }
            "view" => {
                let count: i64 = self
                    .conn
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='view' AND name=?1",
                        [name],
                        |row| row.get(0),
                    )
                    .map_err(|e| format!("checking view existence: {}", e))?;
                Ok(count > 0)
            }
            "schema" => Err("sqlite does not support schemas".into()),
            "database" => Err("sqlite does not support checking database existence".into()),
            _ => Err(format!("unsupported object type '{}' for sqlite", obj_type)),
        }
    }

    fn driver_name(&self) -> &str {
        "sqlite"
    }
}

#[cfg(feature = "postgres")]
pub struct PostgresDb {
    client: postgres::Client,
    in_transaction: bool,
}

#[cfg(feature = "postgres")]
impl PostgresDb {
    pub fn connect(url: &str) -> Result<Self, String> {
        let client = postgres::Client::connect(url, postgres::NoTls)
            .map_err(|e| format!("connecting to postgres: {}", e))?;
        Ok(Self {
            client,
            in_transaction: false,
        })
    }
}

#[cfg(feature = "postgres")]
impl Database for PostgresDb {
    fn ensure_tracking_table(&mut self, table_name: &str) -> Result<(), String> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS \"{}\" (
                seed_set TEXT PRIMARY KEY,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
            sanitize_identifier(table_name)
        );
        self.client
            .execute(&sql, &[])
            .map_err(|e| format!("creating tracking table: {}", e))?;
        Ok(())
    }

    fn is_seed_applied(&mut self, table_name: &str, seed_set: &str) -> Result<bool, String> {
        let sql = format!(
            "SELECT COUNT(*) FROM \"{}\" WHERE seed_set = $1",
            sanitize_identifier(table_name)
        );
        let row = self
            .client
            .query_one(&sql, &[&seed_set])
            .map_err(|e| format!("checking seed status: {}", e))?;
        let count: i64 = row.get(0);
        Ok(count > 0)
    }

    fn mark_seed_applied(&mut self, table_name: &str, seed_set: &str) -> Result<(), String> {
        let sql = format!(
            "INSERT INTO \"{}\" (seed_set) VALUES ($1) ON CONFLICT DO NOTHING",
            sanitize_identifier(table_name)
        );
        self.client
            .execute(&sql, &[&seed_set])
            .map_err(|e| format!("marking seed applied: {}", e))?;
        Ok(())
    }

    fn remove_seed_mark(&mut self, table_name: &str, seed_set: &str) -> Result<(), String> {
        let sql = format!(
            "DELETE FROM \"{}\" WHERE seed_set = $1",
            sanitize_identifier(table_name)
        );
        self.client
            .execute(&sql, &[&seed_set])
            .map_err(|e| format!("removing seed mark: {}", e))?;
        Ok(())
    }

    fn insert_row(
        &mut self,
        table: &str,
        columns: &[String],
        values: &[String],
        auto_id_column: Option<&str>,
    ) -> Result<Option<i64>, String> {
        let col_list: Vec<String> = columns
            .iter()
            .map(|c| format!("\"{}\"", sanitize_identifier(c)))
            .collect();
        let value_list: Vec<String> = values.iter().map(|v| escape_sql_value(v)).collect();

        if let Some(auto_col) = auto_id_column {
            let returning_col = sanitize_identifier(auto_col);
            let sql = format!(
                "INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING COALESCE(CAST(\"{}\" AS BIGINT), 0)",
                sanitize_identifier(table),
                col_list.join(", "),
                value_list.join(", "),
                returning_col
            );
            let row = self
                .client
                .query_one(&sql, &[])
                .map_err(|e| format!("inserting row into '{}': {}", table, e))?;
            let id: i64 = row.get(0);
            Ok(Some(id))
        } else {
            let sql = format!(
                "INSERT INTO \"{}\" ({}) VALUES ({})",
                sanitize_identifier(table),
                col_list.join(", "),
                value_list.join(", "),
            );
            self.client
                .execute(&sql, &[])
                .map_err(|e| format!("inserting row into '{}': {}", table, e))?;
            Ok(None)
        }
    }

    fn row_exists(
        &mut self,
        table: &str,
        unique_columns: &[String],
        unique_values: &[String],
    ) -> Result<bool, String> {
        if unique_columns.is_empty() {
            return Ok(false);
        }
        let conditions: Vec<String> = unique_columns
            .iter()
            .zip(unique_values.iter())
            .map(|(c, v)| format!("\"{}\" = {}", sanitize_identifier(c), escape_sql_value(v)))
            .collect();
        let sql = format!(
            "SELECT COUNT(*) FROM \"{}\" WHERE {}",
            sanitize_identifier(table),
            conditions.join(" AND ")
        );
        let row = self
            .client
            .query_one(&sql, &[])
            .map_err(|e| format!("checking row existence in '{}': {}", table, e))?;
        let count: i64 = row.get(0);
        Ok(count > 0)
    }

    fn delete_rows(&mut self, table: &str) -> Result<u64, String> {
        let sql = format!("DELETE FROM \"{}\"", sanitize_identifier(table));
        let count = self
            .client
            .execute(&sql, &[])
            .map_err(|e| format!("deleting rows from '{}': {}", table, e))?;
        Ok(count)
    }

    fn begin_transaction(&mut self) -> Result<(), String> {
        self.client
            .execute("BEGIN", &[])
            .map_err(|e| format!("beginning transaction: {}", e))?;
        self.in_transaction = true;
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<(), String> {
        if self.in_transaction {
            self.client
                .execute("COMMIT", &[])
                .map_err(|e| format!("committing transaction: {}", e))?;
            self.in_transaction = false;
        }
        Ok(())
    }

    fn rollback_transaction(&mut self) -> Result<(), String> {
        if self.in_transaction {
            self.client
                .execute("ROLLBACK", &[])
                .map_err(|e| format!("rolling back transaction: {}", e))?;
            self.in_transaction = false;
        }
        Ok(())
    }

    fn create_database(&mut self, name: &str) -> Result<(), String> {
        let safe = sanitize_identifier(name);
        let row = self
            .client
            .query_one(
                "SELECT COUNT(*) FROM pg_database WHERE datname = $1",
                &[&safe],
            )
            .map_err(|e| format!("checking database existence: {}", e))?;
        let count: i64 = row.get(0);
        if count == 0 {
            let sql = format!("CREATE DATABASE \"{}\"", safe);
            self.client
                .execute(&sql, &[])
                .map_err(|e| format!("creating database '{}': {}", name, e))?;
        }
        Ok(())
    }

    fn create_schema(&mut self, name: &str) -> Result<(), String> {
        let sql = format!(
            "CREATE SCHEMA IF NOT EXISTS \"{}\"",
            sanitize_identifier(name)
        );
        self.client
            .execute(&sql, &[])
            .map_err(|e| format!("creating schema '{}': {}", name, e))?;
        Ok(())
    }

    fn object_exists(&mut self, obj_type: &str, name: &str) -> Result<bool, String> {
        let sql = match obj_type {
            "table" => {
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1".to_string()
            }
            "view" => {
                "SELECT COUNT(*) FROM information_schema.views WHERE table_name = $1".to_string()
            }
            "schema" => "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1"
                .to_string(),
            "database" => "SELECT COUNT(*) FROM pg_database WHERE datname = $1".to_string(),
            _ => {
                return Err(format!(
                    "unsupported object type '{}' for postgres",
                    obj_type
                ))
            }
        };
        let row = self
            .client
            .query_one(&sql, &[&name])
            .map_err(|e| format!("checking {} existence: {}", obj_type, e))?;
        let count: i64 = row.get(0);
        Ok(count > 0)
    }

    fn driver_name(&self) -> &str {
        "postgres"
    }
}

#[cfg(feature = "mysql")]
pub struct MysqlDb {
    conn: mysql::PooledConn,
    in_transaction: bool,
}

#[cfg(feature = "mysql")]
impl MysqlDb {
    pub fn connect(url: &str) -> Result<Self, String> {
        let pool = mysql::Pool::new(url).map_err(|e| format!("connecting to mysql: {}", e))?;
        let conn = pool
            .get_conn()
            .map_err(|e| format!("getting mysql connection: {}", e))?;
        Ok(Self {
            conn,
            in_transaction: false,
        })
    }
}

#[cfg(feature = "mysql")]
impl Database for MysqlDb {
    fn ensure_tracking_table(&mut self, table_name: &str) -> Result<(), String> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS `{}` (
                seed_set VARCHAR(255) PRIMARY KEY,
                applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
            sanitize_identifier(table_name)
        );
        use mysql::prelude::Queryable;
        self.conn
            .query_drop(&sql)
            .map_err(|e| format!("creating tracking table: {}", e))?;
        Ok(())
    }

    fn is_seed_applied(&mut self, table_name: &str, seed_set: &str) -> Result<bool, String> {
        let sql = format!(
            "SELECT COUNT(*) FROM `{}` WHERE seed_set = ?",
            sanitize_identifier(table_name)
        );
        use mysql::prelude::Queryable;
        let count: Option<i64> = self
            .conn
            .exec_first(&sql, (seed_set,))
            .map_err(|e| format!("checking seed status: {}", e))?;
        Ok(count.unwrap_or(0) > 0)
    }

    fn mark_seed_applied(&mut self, table_name: &str, seed_set: &str) -> Result<(), String> {
        let sql = format!(
            "INSERT IGNORE INTO `{}` (seed_set) VALUES (?)",
            sanitize_identifier(table_name)
        );
        use mysql::prelude::Queryable;
        self.conn
            .exec_drop(&sql, (seed_set,))
            .map_err(|e| format!("marking seed applied: {}", e))?;
        Ok(())
    }

    fn remove_seed_mark(&mut self, table_name: &str, seed_set: &str) -> Result<(), String> {
        let sql = format!(
            "DELETE FROM `{}` WHERE seed_set = ?",
            sanitize_identifier(table_name)
        );
        use mysql::prelude::Queryable;
        self.conn
            .exec_drop(&sql, (seed_set,))
            .map_err(|e| format!("removing seed mark: {}", e))?;
        Ok(())
    }

    fn insert_row(
        &mut self,
        table: &str,
        columns: &[String],
        values: &[String],
        _auto_id_column: Option<&str>,
    ) -> Result<Option<i64>, String> {
        let col_list: Vec<String> = columns
            .iter()
            .map(|c| format!("`{}`", sanitize_identifier(c)))
            .collect();
        let placeholders: Vec<String> = columns.iter().map(|_| "?".into()).collect();
        let sql = format!(
            "INSERT INTO `{}` ({}) VALUES ({})",
            sanitize_identifier(table),
            col_list.join(", "),
            placeholders.join(", ")
        );
        use mysql::prelude::Queryable;
        let params: Vec<mysql::Value> = values
            .iter()
            .map(|v| mysql::Value::from(v.as_str()))
            .collect();
        self.conn
            .exec_drop(&sql, &params)
            .map_err(|e| format!("inserting row into '{}': {}", table, e))?;
        let id: Option<i64> = self
            .conn
            .exec_first("SELECT LAST_INSERT_ID()", ())
            .map_err(|e| format!("getting last insert id: {}", e))?;
        Ok(id)
    }

    fn row_exists(
        &mut self,
        table: &str,
        unique_columns: &[String],
        unique_values: &[String],
    ) -> Result<bool, String> {
        if unique_columns.is_empty() {
            return Ok(false);
        }
        let conditions: Vec<String> = unique_columns
            .iter()
            .map(|c| format!("`{}` = ?", sanitize_identifier(c)))
            .collect();
        let sql = format!(
            "SELECT COUNT(*) FROM `{}` WHERE {}",
            sanitize_identifier(table),
            conditions.join(" AND ")
        );
        use mysql::prelude::Queryable;
        let params: Vec<mysql::Value> = unique_values
            .iter()
            .map(|v| mysql::Value::from(v.as_str()))
            .collect();
        let count: Option<i64> = self
            .conn
            .exec_first(&sql, &params)
            .map_err(|e| format!("checking row existence in '{}': {}", table, e))?;
        Ok(count.unwrap_or(0) > 0)
    }

    fn delete_rows(&mut self, table: &str) -> Result<u64, String> {
        let sql = format!("DELETE FROM `{}`", sanitize_identifier(table));
        use mysql::prelude::Queryable;
        self.conn
            .query_drop(&sql)
            .map_err(|e| format!("deleting rows from '{}': {}", table, e))?;
        let affected: Option<u64> = self
            .conn
            .exec_first("SELECT ROW_COUNT()", ())
            .map_err(|e| format!("getting affected rows: {}", e))?;
        Ok(affected.unwrap_or(0))
    }

    fn begin_transaction(&mut self) -> Result<(), String> {
        use mysql::prelude::Queryable;
        self.conn
            .query_drop("START TRANSACTION")
            .map_err(|e| format!("beginning transaction: {}", e))?;
        self.in_transaction = true;
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<(), String> {
        if self.in_transaction {
            use mysql::prelude::Queryable;
            self.conn
                .query_drop("COMMIT")
                .map_err(|e| format!("committing transaction: {}", e))?;
            self.in_transaction = false;
        }
        Ok(())
    }

    fn rollback_transaction(&mut self) -> Result<(), String> {
        if self.in_transaction {
            use mysql::prelude::Queryable;
            self.conn
                .query_drop("ROLLBACK")
                .map_err(|e| format!("rolling back transaction: {}", e))?;
            self.in_transaction = false;
        }
        Ok(())
    }

    fn create_database(&mut self, name: &str) -> Result<(), String> {
        let sql = format!(
            "CREATE DATABASE IF NOT EXISTS `{}`",
            sanitize_identifier(name)
        );
        use mysql::prelude::Queryable;
        self.conn
            .query_drop(&sql)
            .map_err(|e| format!("creating database '{}': {}", name, e))?;
        Ok(())
    }

    fn create_schema(&mut self, name: &str) -> Result<(), String> {
        // In MySQL, schema and database are synonymous
        self.create_database(name)
    }

    fn object_exists(&mut self, obj_type: &str, name: &str) -> Result<bool, String> {
        use mysql::prelude::Queryable;
        let sql = match obj_type {
            "table" => "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = ?",
            "view" => "SELECT COUNT(*) FROM information_schema.views WHERE table_schema = DATABASE() AND table_name = ?",
            "schema" | "database" => "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = ?",
            _ => return Err(format!("unsupported object type '{}' for mysql", obj_type)),
        };
        let count: Option<i64> = self
            .conn
            .exec_first(sql, (name,))
            .map_err(|e| format!("checking {} existence: {}", obj_type, e))?;
        Ok(count.unwrap_or(0) > 0)
    }

    fn driver_name(&self) -> &str {
        "mysql"
    }
}

pub fn connect(driver: &str, url: &str) -> Result<Box<dyn Database>, String> {
    match driver {
        #[cfg(feature = "sqlite")]
        "sqlite" => Ok(Box::new(SqliteDb::connect(url)?)),
        #[cfg(feature = "postgres")]
        "postgres" | "postgresql" => Ok(Box::new(PostgresDb::connect(url)?)),
        #[cfg(feature = "mysql")]
        "mysql" => Ok(Box::new(MysqlDb::connect(url)?)),
        _ => {
            let mut supported = Vec::new();
            #[cfg(feature = "sqlite")]
            supported.push("sqlite");
            #[cfg(feature = "postgres")]
            supported.push("postgres");
            #[cfg(feature = "mysql")]
            supported.push("mysql");
            Err(format!(
                "unsupported database driver: '{}' (supported: {})",
                driver,
                supported.join(", ")
            ))
        }
    }
}

fn sanitize_identifier(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

fn escape_sql_value(val: &str) -> String {
    format!("'{}'", val.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("users"), "users");
        assert_eq!(sanitize_identifier("my_table"), "my_table");
        assert_eq!(sanitize_identifier("bad;drop"), "baddrop");
        assert_eq!(sanitize_identifier("table--name"), "tablename");
    }

    #[test]
    fn test_sqlite_tracking_table() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        db.ensure_tracking_table("initium_seed").unwrap();
        assert!(!db.is_seed_applied("initium_seed", "test_set").unwrap());
        db.mark_seed_applied("initium_seed", "test_set").unwrap();
        assert!(db.is_seed_applied("initium_seed", "test_set").unwrap());
        db.remove_seed_mark("initium_seed", "test_set").unwrap();
        assert!(!db.is_seed_applied("initium_seed", "test_set").unwrap());
    }

    #[test]
    fn test_sqlite_insert_and_exists() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        db.conn
            .execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT UNIQUE)",
                [],
            )
            .unwrap();

        let columns = vec!["name".into(), "email".into()];
        let values = vec!["Alice".into(), "alice@example.com".into()];
        let id = db.insert_row("users", &columns, &values, None).unwrap();
        assert!(id.is_some());
        assert_eq!(id.unwrap(), 1);

        let unique_cols = vec!["email".into()];
        let unique_vals = vec!["alice@example.com".into()];
        assert!(db.row_exists("users", &unique_cols, &unique_vals).unwrap());

        let unique_vals2 = vec!["bob@example.com".into()];
        assert!(!db.row_exists("users", &unique_cols, &unique_vals2).unwrap());
    }

    #[test]
    fn test_sqlite_delete_rows() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        db.conn
            .execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)", [])
            .unwrap();
        db.insert_row("items", &["name".into()], &["item1".into()], None)
            .unwrap();
        db.insert_row("items", &["name".into()], &["item2".into()], None)
            .unwrap();
        let count = db.delete_rows("items").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_sqlite_transactions() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        db.conn
            .execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", [])
            .unwrap();
        db.begin_transaction().unwrap();
        db.insert_row("t", &["v".into()], &["a".into()], None)
            .unwrap();
        db.rollback_transaction().unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);

        db.begin_transaction().unwrap();
        db.insert_row("t", &["v".into()], &["b".into()], None)
            .unwrap();
        db.commit_transaction().unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_row_exists_empty_unique_key() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        assert!(!db.row_exists("any", &[], &[]).unwrap());
    }

    #[test]
    fn test_connect_unsupported_driver() {
        let result = connect("oracle", "localhost");
        assert!(result.is_err());
    }

    #[test]
    fn test_connect_sqlite() {
        let db = connect("sqlite", ":memory:");
        assert!(db.is_ok());
    }

    #[test]
    fn test_mark_seed_idempotent() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        db.ensure_tracking_table("initium_seed").unwrap();
        db.mark_seed_applied("initium_seed", "set1").unwrap();
        db.mark_seed_applied("initium_seed", "set1").unwrap();
        assert!(db.is_seed_applied("initium_seed", "set1").unwrap());
    }

    #[test]
    fn test_sqlite_object_exists_table() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        assert!(!db.object_exists("table", "users").unwrap());
        db.conn
            .execute("CREATE TABLE users (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        assert!(db.object_exists("table", "users").unwrap());
    }

    #[test]
    fn test_sqlite_object_exists_view() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        db.conn
            .execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)", [])
            .unwrap();
        assert!(!db.object_exists("view", "items_view").unwrap());
        db.conn
            .execute("CREATE VIEW items_view AS SELECT * FROM items", [])
            .unwrap();
        assert!(db.object_exists("view", "items_view").unwrap());
    }

    #[test]
    fn test_sqlite_object_exists_schema_unsupported() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        let result = db.object_exists("schema", "public");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not support schemas"));
    }

    #[test]
    fn test_sqlite_object_exists_database_unsupported() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        let result = db.object_exists("database", "mydb");
        assert!(result.is_err());
    }

    #[test]
    fn test_sqlite_create_database_unsupported() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        let result = db.create_database("mydb");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not support"));
    }

    #[test]
    fn test_sqlite_create_schema_unsupported() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        let result = db.create_schema("myschema");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not support"));
    }

    #[test]
    fn test_sqlite_driver_name() {
        let db = SqliteDb::connect(":memory:").unwrap();
        assert_eq!(db.driver_name(), "sqlite");
    }

    #[test]
    fn test_sqlite_object_exists_unknown_type() {
        let mut db = SqliteDb::connect(":memory:").unwrap();
        let result = db.object_exists("index", "my_index");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported object type"));
    }
}
