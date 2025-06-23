//! Service Layer
//!
//! This module contains business logic and data access operations.

use crate::database::Database;
use crate::models::*;
//use chrono::NaiveDate;
use rusqlite::Result as SqlResult;

/// User service for user-related operations
pub struct UserService<'a> {
    db: &'a Database,
}

impl<'a> UserService<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Get all users
    pub fn get_all_users(&self) -> SqlResult<Vec<User>> {
        let mut stmt = self
            .db
            .connection()
            .prepare("SELECT id, name, surname, subscription_date FROM users ORDER BY id")?;

        let user_iter = stmt.query_map([], |row| {
            Ok(User {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                surname: row.get(2)?,
                subscription_date: row.get(3)?,
            })
        })?;

        let mut users = Vec::new();
        for user in user_iter {
            users.push(user?);
        }

        Ok(users)
    }

    /// Get user by ID
    pub fn get_user_by_id(&self, id: i64) -> SqlResult<Option<User>> {
        let mut stmt = self
            .db
            .connection()
            .prepare("SELECT id, name, surname, subscription_date FROM users WHERE id = ?")?;

        let mut user_iter = stmt.query_map([id], |row| {
            Ok(User {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                surname: row.get(2)?,
                subscription_date: row.get(3)?,
            })
        })?;

        match user_iter.next() {
            Some(user) => Ok(Some(user?)),
            None => Ok(None),
        }
    }

    /// Count total users
    pub fn count_users(&self) -> SqlResult<i64> {
        let mut stmt = self.db.connection().prepare("SELECT COUNT(*) FROM users")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }
}

/// Article service for article-related operations
pub struct ArticleService<'a> {
    db: &'a Database,
}

impl<'a> ArticleService<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Get all articles with author information
    pub fn get_articles_with_authors(&self) -> SqlResult<Vec<ArticleWithAuthor>> {
        let mut stmt = self.db.connection().prepare("
            SELECT
                a.id, a.title, a.user_id, a.writing_date, a.publication_date,
                u.name, u.surname, u.subscription_date,
                COUNT(ar.id) as read_count,
                COUNT(CASE WHEN ar.liked = 1 THEN 1 END) as like_count,
                SUM(ar.clap_count) as total_claps
            FROM articles a
            JOIN users u ON a.user_id = u.id
            LEFT JOIN article_reads ar ON a.id = ar.article_id
            GROUP BY a.id, a.title, a.user_id, a.writing_date, a.publication_date, u.name, u.surname, u.subscription_date
            ORDER BY a.id
        ")?;

        let article_iter = stmt.query_map([], |row| {
            let read_count: i64 = row.get(8)?;
            let like_count: i64 = row.get(9)?;
            let clap_count: i64 = row.get::<_, Option<i64>>(10)?.unwrap_or(0);
            let like_percentage = if read_count > 0 {
                (like_count as f64 / read_count as f64) * 100.0
            } else {
                0.0
            };
            let avg_claps_per_read = if read_count > 0 {
                clap_count as f64 / read_count as f64
            } else {
                0.0
            };

            Ok(ArticleWithAuthor {
                article: Article {
                    id: Some(row.get(0)?),
                    title: row.get(1)?,
                    user_id: row.get(2)?,
                    writing_date: row.get(3)?,
                    publication_date: row.get(4)?,
                },
                author: User {
                    id: Some(row.get(2)?),
                    name: row.get(5)?,
                    surname: row.get(6)?,
                    subscription_date: row.get(7)?,
                },
                read_count,
                like_count,
                clap_count,
                like_percentage,
                avg_claps_per_read,
            })
        })?;

        let mut articles = Vec::new();
        for article in article_iter {
            articles.push(article?);
        }

        Ok(articles)
    }

    /// Get top authors by influence score
    pub fn get_top_authors(&self, limit: usize) -> SqlResult<Vec<AuthorStats>> {
        let mut stmt = self.db.connection().prepare(&format!("
            SELECT
                u.name || ' ' || u.surname as author_name,
                COUNT(DISTINCT a.id) as articles_written,
                COUNT(ar.id) as total_reads,
                COUNT(CASE WHEN ar.liked = 1 THEN 1 END) as total_likes,
                SUM(ar.clap_count) as total_claps,
                ROUND(COUNT(CASE WHEN ar.liked = 1 THEN 1 END) * 100.0 / COUNT(ar.id), 1) as like_percentage,
                ROUND(COUNT(ar.id) * 1.0 / COUNT(DISTINCT a.id), 1) as avg_reads_per_article,
                ROUND(SUM(ar.clap_count) * 1.0 / COUNT(DISTINCT a.id), 1) as avg_claps_per_article,
                ROUND((COUNT(ar.id) * 0.4) + (COUNT(DISTINCT a.id) * 10 * 0.3) + (SUM(ar.clap_count) * 0.3), 1) as influence_score
            FROM users u
            JOIN articles a ON u.id = a.user_id
            LEFT JOIN article_reads ar ON a.id = ar.article_id
            GROUP BY u.id, u.name, u.surname
            ORDER BY influence_score DESC, total_reads DESC, articles_written DESC
            LIMIT {}
        ", limit))?;

        let author_iter = stmt.query_map([], |row| {
            Ok(AuthorStats {
                author_name: row.get(0)?,
                articles_written: row.get(1)?,
                total_reads: row.get(2)?,
                total_likes: row.get(3)?,
                total_claps: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                like_percentage: row.get(5)?,
                avg_reads_per_article: row.get(6)?,
                avg_claps_per_article: row.get(7)?,
                influence_score: row.get(8)?,
            })
        })?;

        let mut authors = Vec::new();
        for author in author_iter {
            authors.push(author?);
        }

        Ok(authors)
    }

    /// Get articles by user ID
    pub fn get_articles_by_user(&self, user_id: i64) -> SqlResult<Vec<Article>> {
        let mut stmt = self.db.connection().prepare(
            "SELECT id, title, user_id, writing_date, publication_date
             FROM articles WHERE user_id = ? ORDER BY writing_date DESC",
        )?;

        let article_iter = stmt.query_map([user_id], |row| {
            Ok(Article {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                user_id: row.get(2)?,
                writing_date: row.get(3)?,
                publication_date: row.get(4)?,
            })
        })?;

        let mut articles = Vec::new();
        for article in article_iter {
            articles.push(article?);
        }

        Ok(articles)
    }
}

/// Task service for task-related operations
pub struct TaskService<'a> {
    db: &'a Database,
}

impl<'a> TaskService<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Get all tasks
    pub fn get_all_tasks(&self) -> SqlResult<Vec<Task>> {
        let mut stmt = self
            .db
            .connection()
            .prepare("SELECT id, title, status FROM tasks ORDER BY id")?;

        let task_iter = stmt.query_map([], |row| {
            let status_str: String = row.get(2)?;
            let status = status_str
                .parse::<TaskStatus>()
                .unwrap_or(TaskStatus::Pending);

            Ok(Task {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                status,
            })
        })?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task?);
        }

        Ok(tasks)
    }
}
