//! validators module - file 52

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidatorsStatus {
    Active,
    Inactive,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorsItem {
    pub id: String,
    pub name: String,
    pub status: ValidatorsStatus,
    pub metadata: Option<HashMap<String, String>>,
}

pub struct ValidatorsService {
    items: HashMap<String, ValidatorsItem>,
}

impl ValidatorsService {
    pub fn new() -> Self {
        Self { items: HashMap::new() }
    }
    
    pub fn create(&mut self, item_id: String, name: String) -> ValidatorsItem {
        let item = ValidatorsItem {
            id: item_id.clone(),
            name,
            status: ValidatorsStatus::Pending,
            metadata: None,
        };
        self.items.insert(item_id, item.clone());
        item
    }
    
    pub fn get(&self, item_id: &str) -> Option<&ValidatorsItem> {
        self.items.get(item_id)
    }
    
    pub fn list_all(&self) -> Vec<&ValidatorsItem> {
        self.items.values().collect()
    }
    
    pub fn update_status(&mut self, item_id: &str, status: ValidatorsStatus) -> bool {
        if let Some(item) = self.items.get_mut(item_id) {
            item.status = status;
            true
        } else {
            false
        }
    }
}

pub fn processValidators(data: HashMap<String, String>) -> HashMap<String, usize> {
    let mut service = ValidatorsService::new();
    for (key, value) in data {
        service.create(key, value);
    }
    let mut result = HashMap::new();
    result.insert("processed".to_string(), service.list_all().len());
    result
}
