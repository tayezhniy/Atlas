//! products module - file 43

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProductsStatus {
    Active,
    Inactive,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductsItem {
    pub id: String,
    pub name: String,
    pub status: ProductsStatus,
    pub metadata: Option<HashMap<String, String>>,
}

pub struct ProductsService {
    items: HashMap<String, ProductsItem>,
}

impl ProductsService {
    pub fn new() -> Self {
        Self { items: HashMap::new() }
    }
    
    pub fn create(&mut self, item_id: String, name: String) -> ProductsItem {
        let item = ProductsItem {
            id: item_id.clone(),
            name,
            status: ProductsStatus::Pending,
            metadata: None,
        };
        self.items.insert(item_id, item.clone());
        item
    }
    
    pub fn get(&self, item_id: &str) -> Option<&ProductsItem> {
        self.items.get(item_id)
    }
    
    pub fn list_all(&self) -> Vec<&ProductsItem> {
        self.items.values().collect()
    }
    
    pub fn update_status(&mut self, item_id: &str, status: ProductsStatus) -> bool {
        if let Some(item) = self.items.get_mut(item_id) {
            item.status = status;
            true
        } else {
            false
        }
    }
}

pub fn processProducts(data: HashMap<String, String>) -> HashMap<String, usize> {
    let mut service = ProductsService::new();
    for (key, value) in data {
        service.create(key, value);
    }
    let mut result = HashMap::new();
    result.insert("processed".to_string(), service.list_all().len());
    result
}
