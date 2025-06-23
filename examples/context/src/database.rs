//! Database Connection and Configuration
//!
//! This module handles the database connection and provides
//! basic database operations.

use crate::models::*;
use rusqlite::{Connection, Result as SqlResult};
use std::path::Path;

/// Database connection wrapper
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Create a new database connection
    pub fn new<P: AsRef<Path>>(db_path: P) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        Ok(Database { conn })
    }

    /// Create an in-memory database for testing
    pub fn in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        Ok(Database { conn })
    }

    /// Initialize database tables (for testing or setup)
    pub fn init_tables(&self) -> SqlResult<()> {
        // Users table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                surname TEXT NOT NULL,
                subscription_date DATE NOT NULL
            )",
            [],
        )?;

        // Tasks table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending'
            )",
            [],
        )?;

        // Articles table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS articles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                user_id INTEGER NOT NULL,
                writing_date DATE NOT NULL,
                publication_date DATE,
                FOREIGN KEY (user_id) REFERENCES users(id)
            )",
            [],
        )?;

        // Article reads table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS article_reads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                article_id INTEGER NOT NULL,
                reader_id INTEGER NOT NULL,
                read_date DATE NOT NULL,
                liked BOOLEAN NOT NULL DEFAULT 0,
                clap_count INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (article_id) REFERENCES articles(id),
                FOREIGN KEY (reader_id) REFERENCES users(id),
                UNIQUE(article_id, reader_id)
            )",
            [],
        )?;

        Ok(())
    }

    /// Execute a raw SQL query and return the number of affected rows
    pub fn execute(&self, sql: &str) -> SqlResult<usize> {
        self.conn.execute(sql, [])
    }

    /// Get a reference to the underlying connection for complex queries
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Begin a transaction
    pub fn begin_transaction(&mut self) -> SqlResult<rusqlite::Transaction> {
        self.conn.transaction()
    }

    /// Get database schema information
    pub fn get_schema(&self) -> SqlResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )?;

        let schema_iter = stmt.query_map([], |row| Ok(row.get::<_, String>(0)?))?;

        let mut schemas = Vec::new();
        for schema in schema_iter {
            schemas.push(schema?);
        }

        Ok(schemas)
    }

    /// Get table names
    pub fn get_table_names(&self) -> SqlResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )?;

        let table_iter = stmt.query_map([], |row| Ok(row.get::<_, String>(0)?))?;

        let mut tables = Vec::new();
        for table in table_iter {
            tables.push(table?);
        }

        Ok(tables)
    }
}

/// Database connection trait for dependency injection
pub trait DatabaseConnection {
    fn get_connection(&self) -> &Connection;
}

impl DatabaseConnection for Database {
    fn get_connection(&self) -> &Connection {
        &self.conn
    }
}
