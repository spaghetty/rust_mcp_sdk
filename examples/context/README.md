# Database Context Example

This example demonstrates how to properly map your database structure to Rust code with a clean, maintainable architecture.

## Features

- **Type-safe database mapping** using Rust structs
- **Service layer** for business logic separation
- **Error handling** with proper Result types
- **Comprehensive examples** of database operations
- **Testing support** with in-memory database
- **Documentation** for all public APIs

## Database Schema

This example maps the following database tables:

### Users
- Stores user information with subscription dates
- Primary key: `id`

### Tasks
- Simple task management with status tracking
- Statuses: pending, in_progress, completed, cancelled

### Articles
- Articles written by users
- Foreign key relationship to users
- Supports draft (unpublished) and published articles

### Article Reads
- Tracks reading activity and user preferences
- Many-to-many relationship between users and articles
- Includes like/dislike functionality
- **NEW**: Tracks clap counts for enhanced article appreciation (0-10+ claps per read)

## Architecture

```
models.rs      -> Data structures (User, Article, Task, etc.)
database.rs    -> Database connection and configuration
services.rs    -> Business logic and data access
main.rs        -> Example application and usage
```

## Usage

### Basic Usage

```rust
use database::Database;
use services::{UserService, ArticleService};

// Connect to database
let db = Database::new("path/to/database.db")?;

// Initialize services
let user_service = UserService::new(&db);
let article_service = ArticleService::new(&db);

// Get all users
let users = user_service.get_all_users()?;

// Get top authors
let top_authors = article_service.get_top_authors(5)?;
```

### Running the Example

```bash
# Run with default database path
cargo run --bin database_example

# Run with custom database path
DATABASE_PATH=/path/to/your/database.db cargo run --bin database_example
```

### Testing

```bash
cargo test
```

## Key Features Demonstrated

1. **Type Safety**: All database fields are mapped to appropriate Rust types
2. **Error Handling**: Proper use of `Result` types for database operations
3. **Business Logic**: Complex queries like author rankings encapsulated in services
4. **Clean Architecture**: Separation of concerns between data, database, and business logic
5. **Documentation**: Comprehensive documentation for maintainability
6. **Enhanced Metrics**: Clap-based engagement tracking for more nuanced content appreciation

## Models

### User
```rust
pub struct User {
    pub id: Option<i64>,
    pub name: String,
    pub surname: String,
    pub subscription_date: NaiveDate,
}
```

### Article
```rust
pub struct Article {
    pub id: Option<i64>,
    pub title: String,
    pub user_id: i64,
    pub writing_date: NaiveDate,
    pub publication_date: Option<NaiveDate>,
}
```

### ArticleRead (Updated with Clap Count)
```rust
pub struct ArticleRead {
    pub id: Option<i64>,
    pub article_id: i64,
    pub reader_id: i64,
    pub read_date: NaiveDate,
    pub liked: bool,
    pub clap_count: i64,  // NEW: Number of claps (0-10+)
}
```

### AuthorStats (Enhanced with Clap Metrics)
```rust
pub struct AuthorStats {
    pub author_name: String,
    pub articles_written: i64,
    pub total_reads: i64,
    pub total_likes: i64,
    pub total_claps: i64,                 // NEW: Total claps received
    pub like_percentage: f64,
    pub avg_reads_per_article: f64,
    pub avg_claps_per_article: f64,       // NEW: Average claps per article
    pub influence_score: f64,             // UPDATED: Now includes clap metrics
}
```

## Enhanced Influence Score Calculation

The influence score now incorporates clap counts for more accurate author ranking:

```sql
influence_score = (total_reads * 0.4) + (articles_written * 10 * 0.3) + (total_claps * 0.3)
```

**Components:**
- **40%** - Reader engagement (total reads)
- **30%** - Content productivity (articles written, scaled by 10)
- **30%** - Appreciation depth (total claps received)

## Services

- **UserService**: User management operations
- **ArticleService**: Article and author statistics with clap metrics
- **TaskService**: Task management operations

Each service provides a clean API for database operations while handling the complexity of SQL queries internally.

## What's New in Clap Count Feature

1. **Enhanced Engagement Tracking**: Beyond simple likes/dislikes, readers can now express varying levels of appreciation (0-10+ claps)
2. **Improved Author Rankings**: Influence scores now consider clap depth, not just read volume
3. **Detailed Analytics**: Track average claps per article and per read for better content insights
4. **Realistic Data Distribution**: Liked articles typically receive 1-10 claps, while disliked articles receive 0-2 claps

The clap count feature provides a more nuanced view of content appreciation, similar to Medium's clapping system, allowing for better content and author analytics.