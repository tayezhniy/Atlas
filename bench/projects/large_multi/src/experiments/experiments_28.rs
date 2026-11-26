//! experiments module - file 28

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExperimentsStatus {
    Active,
    Inactive,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentsItem {
    pub id: String,
    pub name: String,
    pub status: ExperimentsStatus,
    pub metadata: Option<HashMap<String, String>>,
}

pub struct ExperimentsService {
    items: HashMap<String, ExperimentsItem>,
}

impl ExperimentsService {
    pub fn new() -> Self {
        Self { items: HashMap::new() }
    }
    
    pub fn create(&mut self, item_id: String, name: String) -> ExperimentsItem {
        let item = ExperimentsItem {
            id: item_id.clone(),
            name,
            status: ExperimentsStatus::Pending,
            metadata: None,
        };
        self.items.insert(item_id, item.clone());
        item
    }
    
    pub fn get(&self, item_id: &str) -> Option<&ExperimentsItem> {
        self.items.get(item_id)
    }
    
    pub fn list_all(&self) -> Vec<&ExperimentsItem> {
        self.items.values().collect()
    }
    
    pub fn update_status(&mut self, item_id: &str, status: ExperimentsStatus) -> bool {
        if let Some(item) = self.items.get_mut(item_id) {
            item.status = status;
            true
        } else {
            false
        }
    }
}

pub fn processExperiments(data: HashMap<String, String>) -> HashMap<String, usize> {
    let mut service = ExperimentsService::new();
    for (key, value) in data {
        service.create(key, value);
    }
    let mut result = HashMap::new();
    result.insert("processed".to_string(), service.list_all().len());
    result
}
