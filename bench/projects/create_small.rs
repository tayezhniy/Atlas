//! Генератор маленького тестового проекта (~50 файлов)

use std::fs;
use std::path::Path;

fn main() {
    let root = Path::new("bench/projects/small_rust");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::create_dir_all(root.join("benches")).unwrap();

    // Cargo.toml
    fs::write(root.join("Cargo.toml"), r#"
[package]
name = "small_rust"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
"#).unwrap();

    // src/lib.rs
    fs::write(root.join("src/lib.rs"), r#"
//! Small Rust project for benchmarking

pub mod config;
pub mod models;
pub mod services;
pub mod handlers;
pub mod utils;

pub use config::Config;
pub use models::{User, Order, Product};
pub use services::{UserService, OrderService};
pub use handlers::{ApiHandler, WebHandler};
"#).unwrap();

    // src/config.rs
    fs::write(root.join("src/config.rs"), r#"
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub database_url: String,
    pub api_port: u16,
    pub max_connections: usize,
    pub redis_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://app.db".into()),
            api_port: std::env::var("API_PORT").unwrap_or("8080").parse().unwrap_or(8080),
            max_connections: std::env::var("MAX_CONN").unwrap_or("100").parse().unwrap_or(100),
            redis_url: std::env::var("REDIS_URL").ok(),
        }
    }
}
"#).unwrap();

    // src/models.rs
    fs::write(root.join("src/models.rs"), r#"
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub email: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: u64,
    pub name: String,
    pub price: f64,
    pub stock: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub user_id: u64,
    pub items: Vec<OrderItem>,
    pub total: f64,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub product_id: u64,
    pub quantity: u32,
    pub unit_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Shipped,
    Delivered,
    Cancelled,
}
"#).unwrap();

    // src/services.rs
    fs::write(root.join("src/services.rs"), r#"
use crate::models::{User, Product, Order, OrderItem, OrderStatus};
use crate::Config;
use std::collections::HashMap;

pub struct UserService {
    users: HashMap<u64, User>,
    config: Config,
}

impl UserService {
    pub fn new(config: Config) -> Self {
        Self {
            users: HashMap::new(),
            config,
        }
    }

    pub fn create_user(&mut self, email: String, name: String) -> User {
        let id = self.users.len() as u64 + 1;
        let user = User {
            id,
            email,
            name,
            created_at: chrono::Utc::now(),
        };
        self.users.insert(id, user.clone());
        user
    }

    pub fn get_user(&self, id: u64) -> Option<&User> {
        self.users.get(&id)
    }

    pub fn list_users(&self) -> Vec<&User> {
        self.users.values().collect()
    }

    pub fn update_user(&mut self, id: u64, name: String) -> Option<User> {
        if let Some(user) = self.users.get_mut(&id) {
            user.name = name;
            Some(user.clone())
        } else {
            None
        }
    }

    pub fn delete_user(&mut self, id: u64) -> bool {
        self.users.remove(&id).is_some()
    }
}

pub struct OrderService {
    orders: HashMap<u64, Order>,
    products: HashMap<u64, Product>,
}

impl OrderService {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            products: HashMap::new(),
        }
    }

    pub fn add_product(&mut self, product: Product) {
        self.products.insert(product.id, product);
    }

    pub fn create_order(&mut self, user_id: u64, items: Vec<OrderItem>) -> Order {
        let mut total = 0.0;
        for item in &items {
            if let Some(product) = self.products.get(&item.product_id) {
                total += product.price * item.quantity as f64;
            }
        }
        let id = self.orders.len() as u64 + 1;
        let order = Order {
            id,
            user_id,
            items,
            total,
            status: OrderStatus::Pending,
        };
        self.orders.insert(id, order.clone());
        order
    }

    pub fn get_order(&self, id: u64) -> Option<&Order> {
        self.orders.get(&id)
    }

    pub fn confirm_order(&mut self, id: u64) -> bool {
        if let Some(order) = self.orders.get_mut(&id) {
            order.status = OrderStatus::Confirmed;
            true
        } else {
            false
        }
    }

    pub fn ship_order(&mut self, id: u64) -> bool {
        if let Some(order) = self.orders.get_mut(&id) {
            order.status = OrderStatus::Shipped;
            true
        } else {
            false
        }
    }
}
"#).unwrap();

    // src/handlers.rs
    fs::write(root.join("src/handlers.rs"), r#"
use crate::services::{UserService, OrderService};
use crate::models::{User, Order, OrderItem};
use crate::Config;

pub struct ApiHandler {
    user_service: UserService,
    order_service: OrderService,
}

impl ApiHandler {
    pub fn new(config: Config) -> Self {
        Self {
            user_service: UserService::new(config),
            order_service: OrderService::new(),
        }
    }

    pub fn handle_create_user(&mut self, email: String, name: String) -> User {
        self.user_service.create_user(email, name)
    }

    pub fn handle_get_user(&self, id: u64) -> Option<&User> {
        self.user_service.get_user(id)
    }

    pub fn handle_create_order(&mut self, user_id: u64, items: Vec<OrderItem>) -> Order {
        self.order_service.create_order(user_id, items)
    }

    pub fn handle_get_order(&self, id: u64) -> Option<&Order> {
        self.order_service.get_order(id)
    }
}

pub struct WebHandler {
    api: ApiHandler,
}

impl WebHandler {
    pub fn new(config: Config) -> Self {
        Self {
            api: ApiHandler::new(config),
        }
    }

    pub fn render_user_list(&self) -> String {
        let users = self.api.user_service.list_users();
        let mut html = String::from("<ul>");
        for user in users {
            html.push_str(&format!("<li>{} ({})</li>", user.name, user.email));
        }
        html.push_str("</ul>");
        html
    }
}
"#).unwrap();

    // src/utils.rs
    fs::write(root.join("src/utils.rs"), r#"
pub fn format_currency(amount: f64, currency: &str) -> String {
    format!("{}{:.2}", currency, amount)
}

pub fn calculate_tax(amount: f64, rate: f64) -> f64 {
    amount * rate
}

pub fn validate_email(email: &str) -> bool {
    email.contains('@') && email.contains('.')
}

pub fn generate_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}
"#).unwrap();

    // tests/integration.rs
    fs::write(root.join("tests/integration.rs"), r#"
use small_rust::{Config, services::UserService, models::User};

#[test]
fn test_user_creation() {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    let user = service.create_user("test@example.com".into(), "Test User".into());
    assert_eq!(user.email, "test@example.com");
    assert_eq!(user.name, "Test User");
}

#[test]
fn test_user_retrieval() {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    let user = service.create_user("test2@example.com".into(), "Test User 2".into());
    let retrieved = service.get_user(user.id).unwrap();
    assert_eq!(retrieved.email, user.email);
}

#[test]
fn test_user_update() {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    let user = service.create_user("test3@example.com".into(), "Test User 3".into());
    let updated = service.update_user(user.id, "Updated Name".into());
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().name, "Updated Name");
}

#[test]
fn test_user_deletion() {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    let user = service.create_user("test4@example.com".into(), "Test User 4".into());
    let deleted = service.delete_user(user.id);
    assert!(deleted);
    assert!(service.get_user(user.id).is_none());
}
"#).unwrap();

    // benches/benchmark.rs
    fs::write(root.join("benches/benchmark.rs"), r#"
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use small_rust::{Config, services::UserService};

fn bench_user_creation(c: &mut Criterion) {
    c.bench_function("user_creation", |b| {
        b.iter(|| {
            let config = Config::from_env();
            let mut service = UserService::new(config);
            for i in 0..100 {
                service.create_user(
                    format!("user{}@example.com", i),
                    format!("User {}", i)
                );
            }
        })
    });
}

fn bench_user_lookup(c: &mut Criterion) {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    for i in 0..1000 {
        service.create_user(format!("user{}@example.com", i), format!("User {}", i));
    }
    
    c.bench_function("user_lookup", |b| {
        b.iter(|| {
            service.get_user(black_box(500))
        })
    });
}

criterion_group!(benches, bench_user_creation, bench_user_lookup);
criterion_main!(benches);
"#).unwrap();

    // README.md
    fs::write(root.join("README.md"), r#"
# Small Rust Project

Тестовый проект для бенчмарка Atlas (~50 файлов).

## Структура

- `src/lib.rs` - точка входа, реэкспорты
- `src/config.rs` - конфигурация
- `src/models.rs` - модели данных (User, Product, Order)
- `src/services.rs` - бизнес-логика (UserService, OrderService)
- `src/handlers.rs` - HTTP/веб обработчики
- `src/utils.rs` - утилиты
- `tests/integration.rs` - интеграционные тесты
- `benches/benchmark.rs` - бенчмарки

## Задачи для замера

1. Найти определение `UserService::create_user`
2. Найти все вызовы `Config::from_env`
3. Найти импорты `serde`
4. Оценить влияние изменения `OrderService::create_order`
5. Найти тесты для `UserService`
"#).unwrap();

    println!("Created small_rust project at {:?}", root);
}
