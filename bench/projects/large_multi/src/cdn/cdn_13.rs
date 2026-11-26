//! cdn module - file 13

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CdnStatus {
    Active,
    Inactive,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdnItem {
    pub id: String,
    pub name: String,
    pub status: CdnStatus,
    pub metadata: Option<HashMap<String, String>>,
}

pub struct CdnService {
    items: HashMap<String, CdnItem>,
}

impl CdnService {
    pub fn new() -> Self {
        Self { items: HashMap::new() }
    }
    
    pub fn create(&mut self, item_id: String, name: String) -> CdnItem {
        let item = CdnItem {
            id: item_id.clone(),
            name,
            status: CdnStatus::Pending,
            metadata: None,
        };
        self.items.insert(item_id, item.clone());
        item
    }
    
    pub fn get(&self, item_id: &str) -> Option<&CdnItem> {
        self.items.get(item_id)
    }
    
    pub fn list_all(&self) -> Vec<&CdnItem> {
        self.items.values().collect()
    }
    
    pub fn update_status(&mut self, item_id: &str, status: CdnStatus) -> bool {
        if let Some(item) = self.items.get_mut(item_id) {
            item.status = status;
            true
        } else {
            false
        }
    }
}

pub fn processCdn(data: HashMap<String, String>) -> HashMap<String, usize> {
    let mut service = CdnService::new();
    for (key, value) in data {
        service.create(key, value);
    }
    let mut result = HashMap::new();
    result.insert("processed".to_string(), service.list_all().len());
    result
}
