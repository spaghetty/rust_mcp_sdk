//! Data Models
//!
//! This module contains all the data structures that map to our database tables.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Represents a user in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<i64>,
    pub name: String,
    pub surname: String,
    pub subscription_date: NaiveDate,
}

/// Represents a task in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Option<i64>,
    pub title: String,
    pub status: TaskStatus,
}

/// Task status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "cancelled" => Ok(TaskStatus::Cancelled),
            _ => Err(format!("Invalid task status: {}", s)),
        }
    }
}

/// Represents an article in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: Option<i64>,
    pub title: String,
    pub user_id: i64,
    pub writing_date: NaiveDate,
    pub publication_date: Option<NaiveDate>,
}

/// Represents an article read record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleRead {
    pub id: Option<i64>,
    pub article_id: i64,
    pub reader_id: i64,
    pub read_date: NaiveDate,
    pub liked: bool,
    pub clap_count: i64,
}

/// Author statistics for ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorStats {
    pub author_name: String,
    pub articles_written: i64,
    pub total_reads: i64,
    pub total_likes: i64,
    pub total_claps: i64,
    pub like_percentage: f64,
    pub avg_reads_per_article: f64,
    pub avg_claps_per_article: f64,
    pub influence_score: f64,
}

/// Article with author information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleWithAuthor {
    pub article: Article,
    pub author: User,
    pub read_count: i64,
    pub like_count: i64,
    pub clap_count: i64,
    pub like_percentage: f64,
    pub avg_claps_per_read: f64,
}
