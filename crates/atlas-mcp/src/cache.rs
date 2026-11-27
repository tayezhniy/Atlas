/// Кэш контекста между вызовами в сессии (Optimization 4).
/// Хранит результаты дорогих операций: context_for, neighbors, search.
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct CachedContext {
    pub symbol: String,
    pub path: Option<String>,
    pub text: String,
    pub tokens: usize,
    pub node_ids: Vec<String>,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone, Default)]
pub struct SessionCache {
    /// Кэш context_for по ключу "symbol:path"
    context_for: HashMap<String, CachedContext>,
    /// Кэш neighbors по ключу "symbol:path" или "file:path"
    neighbors: HashMap<String, CachedContext>,
    /// Кэш search по ключу "query:limit"
    search: HashMap<String, CachedContext>,
    /// Кэш overview по ключу "path" (пустой = корень)
    overview: HashMap<String, CachedContext>,
    /// Максимальный размер кэша на сессию
    max_entries: usize,
    /// TTL кэша в секундах
    ttl_secs: u64,
}

impl SessionCache {
    pub fn new(max_entries: usize, ttl_secs: u64) -> Self {
        Self {
            max_entries,
            ttl_secs,
            ..Default::default()
        }
    }

    fn make_key(prefix: &str, args: &str) -> String {
        format!("{}:{}", prefix, args.replace(' ', "_"))
    }

    fn is_expired(&self, cached: &CachedContext) -> bool {
        cached.timestamp.elapsed().as_secs() > self.ttl_secs
    }

    fn evict_if_needed(map: &mut HashMap<String, CachedContext>, max_entries: usize) {
        if map.len() >= max_entries {
            // Удаляем самый старый
            if let Some(oldest_key) = map
                .iter()
                .min_by_key(|(_, v)| v.timestamp)
                .map(|(k, _)| k.clone())
            {
                map.remove(&oldest_key);
            }
        }
    }

    pub fn get_context_for(&self, symbol: &str, path: Option<&str>) -> Option<CachedContext> {
        let key = Self::make_key("ctx", &format!("{}:{}", symbol, path.unwrap_or("")));
        self.context_for.get(&key).and_then(|c| {
            if self.is_expired(c) { None } else { Some(c.clone()) }
        })
    }

    pub fn set_context_for(&mut self, symbol: &str, path: Option<&str>, ctx: CachedContext) {
        let key = Self::make_key("ctx", &format!("{}:{}", symbol, path.unwrap_or("")));
        Self::evict_if_needed(&mut self.context_for, self.max_entries);
        self.context_for.insert(key, ctx);
    }

    pub fn get_neighbors(&self, symbol: Option<&str>, path: Option<&str>) -> Option<CachedContext> {
        let key = match (symbol, path) {
            (Some(s), Some(p)) => Self::make_key("nbr", &format!("{}:{}", s, p)),
            (Some(s), None) => Self::make_key("nbr", &format!("sym:{}", s)),
            (None, Some(p)) => Self::make_key("nbr", &format!("file:{}", p)),
            (None, None) => return None,
        };
        self.neighbors.get(&key).and_then(|c| {
            if self.is_expired(c) { None } else { Some(c.clone()) }
        })
    }

    pub fn set_neighbors(&mut self, symbol: Option<&str>, path: Option<&str>, ctx: CachedContext) {
        let key = match (symbol, path) {
            (Some(s), Some(p)) => Self::make_key("nbr", &format!("{}:{}", s, p)),
            (Some(s), None) => Self::make_key("nbr", &format!("sym:{}", s)),
            (None, Some(p)) => Self::make_key("nbr", &format!("file:{}", p)),
            (None, None) => return,
        };
        Self::evict_if_needed(&mut self.neighbors, self.max_entries);
        self.neighbors.insert(key, ctx);
    }

    pub fn get_search(&self, query: &str, limit: usize) -> Option<CachedContext> {
        let key = Self::make_key("search", &format!("{}:{}", query, limit));
        self.search.get(&key).and_then(|c| {
            if self.is_expired(c) { None } else { Some(c.clone()) }
        })
    }

    pub fn set_search(&mut self, query: &str, limit: usize, ctx: CachedContext) {
        let key = Self::make_key("search", &format!("{}:{}", query, limit));
        Self::evict_if_needed(&mut self.search, self.max_entries);
        self.search.insert(key, ctx);
    }

    pub fn get_overview(&self, path: Option<&str>) -> Option<CachedContext> {
        let key = Self::make_key("overview", path.unwrap_or("root"));
        self.overview.get(&key).and_then(|c| {
            if self.is_expired(c) { None } else { Some(c.clone()) }
        })
    }

    pub fn set_overview(&mut self, path: Option<&str>, ctx: CachedContext) {
        let key = Self::make_key("overview", path.unwrap_or("root"));
        Self::evict_if_needed(&mut self.overview, self.max_entries);
        self.overview.insert(key, ctx);
    }

    /// Очистить устаревшие записи
    pub fn cleanup(&mut self) {
        let now = std::time::Instant::now();
        for map in [&mut self.context_for, &mut self.neighbors, &mut self.search, &mut self.overview] {
            map.retain(|_, v| now.duration_since(v.timestamp).as_secs() <= self.ttl_secs);
        }
    }

    /// Статистика кэша для отладки
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            context_for: self.context_for.len(),
            neighbors: self.neighbors.len(),
            search: self.search.len(),
            overview: self.overview.len(),
            total_tokens: self.context_for.values().map(|c| c.tokens).sum::<usize>()
                + self.neighbors.values().map(|c| c.tokens).sum::<usize>()
                + self.search.values().map(|c| c.tokens).sum::<usize>()
                + self.overview.values().map(|c| c.tokens).sum::<usize>(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub context_for: usize,
    pub neighbors: usize,
    pub search: usize,
    pub overview: usize,
    pub total_tokens: usize,
}

/// Глобальный реестр кэшей по сессиям
pub struct CacheRegistry {
    caches: RwLock<HashMap<String, Arc<RwLock<SessionCache>>>>,
    default_max_entries: usize,
    default_ttl_secs: u64,
}

impl CacheRegistry {
    pub fn new(default_max_entries: usize, default_ttl_secs: u64) -> Self {
        Self {
            caches: RwLock::new(HashMap::new()),
            default_max_entries,
            default_ttl_secs,
        }
    }

    pub fn get_or_create(&self, session_id: &str) -> Arc<RwLock<SessionCache>> {
        let mut caches = self.caches.write().unwrap();
        caches
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(SessionCache::new(
                self.default_max_entries,
                self.default_ttl_secs,
            ))))
            .clone()
    }

    pub fn remove(&self, session_id: &str) {
        self.caches.write().unwrap().remove(session_id);
    }

    pub fn cleanup_all(&self) {
        let caches = self.caches.write().unwrap();
        for cache in caches.values() {
            cache.write().unwrap().cleanup();
        }
    }
}

impl Default for CacheRegistry {
    fn default() -> Self {
        Self::new(100, 300) // 100 записей, 5 минут TTL
    }
}