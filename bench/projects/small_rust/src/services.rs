
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
