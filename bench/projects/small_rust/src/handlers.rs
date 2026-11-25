
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
