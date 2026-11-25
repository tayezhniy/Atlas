#!/usr/bin/env python3
"""Создание большого проекта (~5000+ файлов) для бенчмарка - детерминированная версия"""

import os
import random

# Фиксируем seed для воспроизводимости
random.seed(42)

root = "bench/projects/large_multi"
# Очищаем
import shutil
if os.path.exists(root):
    shutil.rmtree(root)

os.makedirs(os.path.join(root, "src"), exist_ok=True)
os.makedirs(os.path.join(root, "tests"), exist_ok=True)
os.makedirs(os.path.join(root, "docs"), exist_ok=True)

# Домены с фиксированными языками для воспроизводимости
domains = [
    ("billing", "typescript"),
    ("customers", "python"),
    ("products", "rust"),
    ("orders", "typescript"),
    ("inventory", "python"),
    ("shipping", "rust"),
    ("returns", "typescript"),
    ("payments", "python"),
    ("subscriptions", "rust"),
    ("invoices", "typescript"),
    ("notifications", "python"),
    ("analytics", "rust"),
    ("reporting", "typescript"),
    ("audit", "python"),
    ("compliance", "rust"),
    ("users", "typescript"),
    ("roles", "python"),
    ("permissions", "rust"),
    ("organizations", "typescript"),
    ("teams", "python"),
    ("projects", "rust"),
    ("tasks", "typescript"),
    ("comments", "python"),
    ("attachments", "rust"),
    ("workflows", "typescript"),
    ("integrations", "python"),
    ("webhooks", "rust"),
    ("api_keys", "typescript"),
    ("rate_limits", "python"),
    ("quotas", "rust"),
    ("search", "typescript"),
    ("indexing", "python"),
    ("caching", "rust"),
    ("queue", "typescript"),
    ("scheduler", "python"),
    ("monitoring", "rust"),
    ("logging", "typescript"),
    ("metrics", "python"),
    ("tracing", "rust"),
    ("alerts", "typescript"),
    ("feature_flags", "python"),
    ("experiments", "rust"),
    ("rollouts", "typescript"),
    ("config", "python"),
    ("secrets", "rust"),
    ("encryption", "typescript"),
    ("signing", "python"),
    ("verification", "rust"),
    ("tokens", "typescript"),
    ("sessions", "python"),
    ("oauth", "rust"),
    ("sso", "typescript"),
    ("ldap", "python"),
    ("saml", "rust"),
    ("mfa", "typescript"),
    ("email", "python"),
    ("sms", "rust"),
    ("push", "typescript"),
    ("in_app", "python"),
    ("templates", "rust"),
    ("files", "typescript"),
    ("storage", "python"),
    ("cdn", "rust"),
    ("uploads", "typescript"),
    ("processing", "python"),
    ("etl", "rust"),
    ("pipelines", "typescript"),
    ("transforms", "python"),
    ("validators", "rust"),
    ("normalizers", "typescript"),
    ("aggregators", "python"),
    ("enrichers", "rust"),
    ("deduplicators", "typescript"),
    ("routers", "python"),
    ("filters", "rust"),
]

extensions = {"python": ".py", "typescript": ".ts", "rust": ".rs"}

# Создаём структуру
domain_langs = {}
FILES_PER_DOMAIN = 70  # 75 domains * 70 = 5250 files
for domain, lang in domains:
    domain_langs[domain] = lang
    ext = extensions[lang]
    domain_path = os.path.join(root, "src", domain)
    os.makedirs(domain_path, exist_ok=True)
    
    # 70 файлов на домен = 75 * 70 = 5250 файлов
    for i in range(FILES_PER_DOMAIN):
        fname = f"{domain}_{i}{ext}"
        fpath = os.path.join(domain_path, fname)
        
        service_name = domain.capitalize() + "Service"
        item_name = domain.capitalize() + "Item"
        status_name = domain.capitalize() + "Status"
        process_name = "process" + domain.capitalize()
        
        if lang == "python":
            with open(fpath, "w") as f:
                f.write(f'''"""
{domain} module - file {i}
"""

from typing import List, Optional, Dict
from dataclasses import dataclass
from enum import Enum

class {status_name}(Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"
    PENDING = "pending"

@dataclass
class {item_name}:
    id: str
    name: str
    status: {status_name} = {status_name}.PENDING
    metadata: Optional[Dict] = None

class {service_name}:
    def __init__(self):
        self.items: List[{item_name}] = []
    
    def create(self, item_id: str, name: str) -> {item_name}:
        item = {item_name}(id=item_id, name=name)
        self.items.append(item)
        return item
    
    def get(self, item_id: str) -> Optional[{item_name}]:
        for item in self.items:
            if item.id == item_id:
                return item
        return None
    
    def list_all(self) -> List[{item_name}]:
        return self.items.copy()
    
    def update_status(self, item_id: str, status: {status_name}) -> bool:
        item = self.get(item_id)
        if item:
            item.status = status
            return True
        return False

def {process_name}(data: Dict[str, str]) -> Dict[str, int]:
    """Process {domain} data"""
    service = {service_name}()
    for key, value in data.items():
        service.create(key, value)
    return {{"processed": len(service.items)}}
''')
        elif lang == "typescript":
            with open(fpath, "w") as f:
                f.write(f'''/**
 * {domain} module - file {i}
 */

export enum {status_name} {{
    ACTIVE = "active",
    INACTIVE = "inactive",
    PENDING = "pending"
}}

export interface {item_name} {{
    id: string;
    name: string;
    status: {status_name};
    metadata?: Record<string, unknown>;
}}

export class {service_name} {{
    private items: {item_name}[] = [];
    
    create(itemId: string, name: string): {item_name} {{
        const item: {item_name} = {{ id: itemId, name, status: {status_name}.PENDING }};
        this.items.push(item);
        return item;
    }}
    
    get(itemId: string): {item_name} | undefined {{
        return this.items.find(item => item.id === itemId);
    }}
    
    listAll(): {item_name}[] {{
        return [...this.items];
    }}
    
    updateStatus(itemId: string, status: {status_name}): boolean {{
        const item = this.get(itemId);
        if (item) {{
            item.status = status;
            return true;
        }}
        return false;
    }}
}}

export function {process_name}(data: Record<string, unknown>): {{ processed: number }} {{
    const service = new {service_name}();
    for (const [key, value] of Object.entries(data)) {{
        service.create(key, String(value));
    }}
    return {{ processed: service.listAll().length }};
}}
''')
        else:  # rust
            with open(fpath, "w") as f:
                f.write(f'''//! {domain} module - file {i}

use std::collections::HashMap;
use serde::{{Deserialize, Serialize}};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum {status_name} {{
    Active,
    Inactive,
    Pending,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {item_name} {{
    pub id: String,
    pub name: String,
    pub status: {status_name},
    pub metadata: Option<HashMap<String, String>>,
}}

pub struct {service_name} {{
    items: HashMap<String, {item_name}>,
}}

impl {service_name} {{
    pub fn new() -> Self {{
        Self {{ items: HashMap::new() }}
    }}
    
    pub fn create(&mut self, item_id: String, name: String) -> {item_name} {{
        let item = {item_name} {{
            id: item_id.clone(),
            name,
            status: {status_name}::Pending,
            metadata: None,
        }};
        self.items.insert(item_id, item.clone());
        item
    }}
    
    pub fn get(&self, item_id: &str) -> Option<&{item_name}> {{
        self.items.get(item_id)
    }}
    
    pub fn list_all(&self) -> Vec<&{item_name}> {{
        self.items.values().collect()
    }}
    
    pub fn update_status(&mut self, item_id: &str, status: {status_name}) -> bool {{
        if let Some(item) = self.items.get_mut(item_id) {{
            item.status = status;
            true
        }} else {{
            false
        }}
    }}
}}

pub fn {process_name}(data: HashMap<String, String>) -> HashMap<String, usize> {{
    let mut service = {service_name}::new();
    for (key, value) in data {{
        service.create(key, value);
    }}
    let mut result = HashMap::new();
    result.insert("processed".to_string(), service.list_all().len());
    result
}}
''')

# Cargo.toml
with open(os.path.join(root, "Cargo.toml"), "w") as f:
    f.write("""
[workspace]
members = ["src/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
""")

# pyproject.toml
with open(os.path.join(root, "pyproject.toml"), "w") as f:
    f.write("""
[project]
name = "large_multi"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = [
    "pydantic>=2.0",
    "fastapi>=0.100",
]

[tool.ruff]
line-length = 100
target-version = "py310"
""")

# package.json
with open(os.path.join(root, "package.json"), "w") as f:
    f.write("""
{
  "name": "large_multi",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "build": "tsc",
    "test": "jest"
  },
  "devDependencies": {
    "typescript": "^5.0",
    "jest": "^29.0",
    "@types/jest": "^29.0"
  }
}
""")

# README
with open(os.path.join(root, "README.md"), "w") as f:
    f.write(f"""
# Large Multi-Language Project

Тестовый проект для бенчмарка Atlas (~{{len(domains) * FILES_PER_DOMAIN}} файлов).

## Домены ({len(domains)})

{', '.join([d for d, _ in domains])}

## Языки

- Python
- TypeScript  
- Rust

## Задачи для замера

1. Найти все сервисы в домене billing (TypeScript)
2. Найти функцию processBilling в домене billing
3. Получить контекст OrdersService в домене orders (TypeScript)
4. Получить обзор проекта (overview)
5. Статус индекса
""")

print(f"Created large_multi project at {root} with {len(domains)} domains x {FILES_PER_DOMAIN} files = {len(domains) * FILES_PER_DOMAIN} files")
