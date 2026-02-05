use crate::models::ColumnDef;
use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> Result<DbPool> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(Arc::new(Mutex::new(conn)))
}

fn validate_identifier(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "Identifier cannot be empty".into(),
        ));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "Invalid identifier: {}",
            name
        )));
    }
    if name.chars().next().unwrap().is_numeric() {
        return Err(rusqlite::Error::InvalidParameterName(
            "Identifier cannot start with a number".into(),
        ));
    }
    Ok(())
}

pub fn list_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>>>()?;
    Ok(tables)
}

pub fn create_table(conn: &Connection, name: &str, columns: &[ColumnDef]) -> Result<()> {
    validate_identifier(name)?;
    if columns.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "At least one column required".into(),
        ));
    }
    for col in columns {
        validate_identifier(&col.name)?;
        validate_identifier(&col.col_type)?;
    }

    let cols: Vec<String> = columns
        .iter()
        .map(|c| format!("\"{}\" {}", c.name, c.col_type))
        .collect();
    let sql = format!("CREATE TABLE \"{}\" ({})", name, cols.join(", "));
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn drop_table(conn: &Connection, name: &str) -> Result<()> {
    validate_identifier(name)?;
    let sql = format!("DROP TABLE IF EXISTS \"{}\"", name);
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn list_columns(conn: &Connection, table_name: &str) -> Result<Vec<ColumnDef>> {
    validate_identifier(table_name)?;
    let sql = format!("PRAGMA table_info(\"{}\")", table_name);
    let mut stmt = conn.prepare(&sql)?;
    let columns = stmt
        .query_map([], |row| {
            Ok(ColumnDef {
                name: row.get(1)?,
                col_type: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(columns)
}

pub fn add_column(conn: &Connection, table_name: &str, column: &ColumnDef) -> Result<()> {
    validate_identifier(table_name)?;
    validate_identifier(&column.name)?;
    validate_identifier(&column.col_type)?;
    let sql = format!(
        "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}",
        table_name, column.name, column.col_type
    );
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn drop_column(conn: &Connection, table_name: &str, column_name: &str) -> Result<()> {
    validate_identifier(table_name)?;
    validate_identifier(column_name)?;
    let sql = format!(
        "ALTER TABLE \"{}\" DROP COLUMN \"{}\"",
        table_name, column_name
    );
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn get_table_rows(
    conn: &Connection,
    table_name: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<Vec<String>>> {
    validate_identifier(table_name)?;
    let sql = format!(
        "SELECT * FROM \"{}\" LIMIT {} OFFSET {}",
        table_name, limit, offset
    );
    let mut stmt = conn.prepare(&sql)?;
    let col_count = stmt.column_count();
    let rows = stmt
        .query_map([], |row| {
            let mut values = Vec::new();
            for i in 0..col_count {
                let val: String = row
                    .get::<_, rusqlite::types::Value>(i)
                    .map(|v| format!("{:?}", v))
                    .unwrap_or_default();
                values.push(val);
            }
            Ok(values)
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_list_tables_empty() {
        let conn = setup_db();
        let tables = list_tables(&conn).unwrap();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_create_table() {
        let conn = setup_db();
        create_table(
            &conn,
            "users",
            &[
                ColumnDef {
                    name: "id".into(),
                    col_type: "INTEGER".into(),
                },
                ColumnDef {
                    name: "name".into(),
                    col_type: "TEXT".into(),
                },
            ],
        )
        .unwrap();
        let tables = list_tables(&conn).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0], "users");
    }

    #[test]
    fn test_drop_table() {
        let conn = setup_db();
        create_table(
            &conn,
            "users",
            &[ColumnDef {
                name: "id".into(),
                col_type: "INTEGER".into(),
            }],
        )
        .unwrap();
        drop_table(&conn, "users").unwrap();
        let tables = list_tables(&conn).unwrap();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_list_columns() {
        let conn = setup_db();
        create_table(
            &conn,
            "users",
            &[
                ColumnDef {
                    name: "id".into(),
                    col_type: "INTEGER".into(),
                },
                ColumnDef {
                    name: "name".into(),
                    col_type: "TEXT".into(),
                },
            ],
        )
        .unwrap();
        let cols = list_columns(&conn, "users").unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].name, "id");
        assert_eq!(cols[1].name, "name");
    }

    #[test]
    fn test_add_column() {
        let conn = setup_db();
        create_table(
            &conn,
            "users",
            &[ColumnDef {
                name: "id".into(),
                col_type: "INTEGER".into(),
            }],
        )
        .unwrap();
        add_column(
            &conn,
            "users",
            &ColumnDef {
                name: "email".into(),
                col_type: "TEXT".into(),
            },
        )
        .unwrap();
        let cols = list_columns(&conn, "users").unwrap();
        assert_eq!(cols.len(), 2);
    }

    #[test]
    fn test_drop_column() {
        let conn = setup_db();
        create_table(
            &conn,
            "users",
            &[
                ColumnDef {
                    name: "id".into(),
                    col_type: "INTEGER".into(),
                },
                ColumnDef {
                    name: "name".into(),
                    col_type: "TEXT".into(),
                },
            ],
        )
        .unwrap();
        drop_column(&conn, "users", "name").unwrap();
        let cols = list_columns(&conn, "users").unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].name, "id");
    }

    #[test]
    fn test_sql_injection_prevention_table_name() {
        let conn = setup_db();
        let result = create_table(
            &conn,
            "users; DROP TABLE users;--",
            &[ColumnDef {
                name: "id".into(),
                col_type: "INTEGER".into(),
            }],
        );
        assert!(result.is_err());
    }
}
