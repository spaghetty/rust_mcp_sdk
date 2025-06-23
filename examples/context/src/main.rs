//! Main Example Application
//!
//! This demonstrates how to use the database mapping and services
//! with your actual database structure.

mod database;
mod models;
mod services;

use database::Database;
use services::{ArticleService, TaskService, UserService};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get database path from environment or use default
    let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| "database.db".to_string());

    println!("🚀 Database Context Example");
    println!("📁 Connecting to database: {}", db_path);

    // Connect to database
    let db = Database::new(&db_path)?;

    // Initialize tables and sample data if needed
    let table_names = db.get_table_names().unwrap_or_default();
    if table_names.is_empty() || !table_names.contains(&"users".to_string()) {
        println!("🔧 Initializing database with sample data...");
        db.init_tables()?;
        create_sample_data(&db)?;
        println!("✅ Database initialized successfully!");
    }

    // Initialize services
    let user_service = UserService::new(&db);
    let article_service = ArticleService::new(&db);
    let task_service = TaskService::new(&db);

    println!("\n📊 Database Statistics:");

    // Show user statistics
    let user_count = user_service.count_users()?;
    println!("👥 Total Users: {}", user_count);

    // Show some sample users
    let users = user_service.get_all_users()?;
    println!("\n👤 Sample Users:");
    for user in users.iter().take(5) {
        println!(
            "  • {} {} (ID: {}, Subscribed: {})",
            user.name,
            user.surname,
            user.id.unwrap_or(0),
            user.subscription_date
        );
    }

    // Show top authors
    println!("\n🏆 Top 5 Authors by Influence:");
    let top_authors = article_service.get_top_authors(5)?;
    for (i, author) in top_authors.iter().enumerate() {
        println!("  {}. {} - {} articles, {} reads, {} claps, {:.1}% likes, {:.1} claps/article (Score: {:.1})",
            i + 1,
            author.author_name,
            author.articles_written,
            author.total_reads,
            author.total_claps,
            author.like_percentage,
            author.avg_claps_per_article,
            author.influence_score
        );
    }

    // Show articles with authors
    println!("\n📚 Recent Articles:");
    let articles = article_service.get_articles_with_authors()?;
    for article in articles.iter().take(10) {
        let status = if article.article.publication_date.is_some() {
            "Published"
        } else {
            "Draft"
        };

        println!(
            "  • \"{}\" by {} {} - {} reads, {} likes ({:.1}%), {} claps ({:.1} claps/read) [{}]",
            article.article.title,
            article.author.name,
            article.author.surname,
            article.read_count,
            article.like_count,
            article.like_percentage,
            article.clap_count,
            article.avg_claps_per_read,
            status
        );
    }

    // Show tasks
    println!("\n✅ Tasks:");
    let tasks = task_service.get_all_tasks()?;
    for task in tasks.iter().take(5) {
        println!(
            "  • [{}] {} (ID: {})",
            task.status,
            task.title,
            task.id.unwrap_or(0)
        );
    }

    // Demonstrate specific queries
    println!("\n🔍 Detailed Analysis:");

    if let Some(first_user) = users.first() {
        if let Some(user_id) = first_user.id {
            let user_articles = article_service.get_articles_by_user(user_id)?;
            println!(
                "📝 Articles by {} {}: {}",
                first_user.name,
                first_user.surname,
                user_articles.len()
            );

            for article in user_articles.iter().take(3) {
                println!(
                    "  • \"{}\" (Written: {})",
                    article.title, article.writing_date
                );
            }
        }
    }

    // Show database schema
    println!("\n🗄️ Database Schema:");
    let schemas = db.get_schema()?;
    for schema in schemas {
        println!("  {}", schema);
    }

    println!("\n✨ Example completed successfully!");

    Ok(())
}

fn create_sample_data(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    // Insert sample users
    db.execute(
        "INSERT INTO users (name, surname, subscription_date) VALUES
        ('John', 'Smith', '2023-01-15'),
        ('Amanda', 'Davis', '2023-02-22'),
        ('Michael', 'Williams', '2023-03-08'),
        ('Sarah', 'Brown', '2023-03-18'),
        ('David', 'Jones', '2023-04-12')",
    )?;

    // Insert sample articles
    db.execute(
        "INSERT INTO articles (title, user_id, writing_date, publication_date) VALUES
        ('The Future of Technology', 1, '2023-01-20', '2023-01-25'),
        ('Climate Change Solutions', 2, '2023-02-10', '2023-02-15'),
        ('Understanding Machine Learning', 3, '2023-02-25', '2023-03-01'),
        ('Digital Marketing Trends', 4, '2023-03-05', '2023-03-10'),
        ('Sustainable Living Tips', 5, '2023-03-15', '2023-03-20')",
    )?;

    // Insert sample article reads with clap counts
    db.execute(
        "INSERT INTO article_reads (article_id, reader_id, read_date, liked, clap_count) VALUES
        (1, 2, '2023-01-26', 1, 5),
        (1, 3, '2023-01-27', 1, 8),
        (1, 4, '2023-01-28', 0, 1),
        (2, 1, '2023-02-16', 1, 7),
        (2, 3, '2023-02-17', 1, 6),
        (2, 5, '2023-02-18', 1, 9),
        (3, 1, '2023-03-02', 1, 10),
        (3, 2, '2023-03-03', 1, 4),
        (3, 4, '2023-03-04', 0, 2),
        (4, 1, '2023-03-11', 1, 6),
        (4, 2, '2023-03-12', 1, 7),
        (5, 1, '2023-03-21', 1, 8),
        (5, 2, '2023-03-22', 1, 5),
        (5, 3, '2023-03-23', 0, 1)",
    )?;

    // Insert sample tasks
    db.execute(
        "INSERT INTO tasks (title, status) VALUES
        ('Setup database schema', 'completed'),
        ('Implement user authentication', 'pending'),
        ('Add clap count feature', 'completed'),
        ('Write documentation', 'in_progress')",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    //use chrono::NaiveDate;

    #[test]
    fn test_database_connection() {
        let db = Database::in_memory().expect("Failed to create in-memory database");
        db.init_tables().expect("Failed to initialize tables");

        let table_names = db.get_table_names().expect("Failed to get table names");
        assert!(table_names.contains(&"users".to_string()));
        assert!(table_names.contains(&"articles".to_string()));
        assert!(table_names.contains(&"tasks".to_string()));
        assert!(table_names.contains(&"article_reads".to_string()));
    }

    #[test]
    fn test_user_service() {
        let db = Database::in_memory().expect("Failed to create in-memory database");
        db.init_tables().expect("Failed to initialize tables");

        // Insert test user
        db.execute("INSERT INTO users (name, surname, subscription_date) VALUES ('Test', 'User', '2024-01-01')")
            .expect("Failed to insert test user");

        let user_service = UserService::new(&db);
        let users = user_service.get_all_users().expect("Failed to get users");

        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "Test");
        assert_eq!(users[0].surname, "User");
    }
}
