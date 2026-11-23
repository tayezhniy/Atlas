/// Журнал вызовов нейросети (раздел 7.8, 10.3 ТЗ).
use atlas_store::Store;

pub struct CallLogger {
    store: Store,
}

impl CallLogger {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn log_call(&self, session_id: &str, tool: &str, tokens_out: usize) -> anyhow::Result<()> {
        let _ = session_id;
        let _ = tool;
        let _ = tokens_out;
        Ok(())
    }
}