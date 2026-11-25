
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
