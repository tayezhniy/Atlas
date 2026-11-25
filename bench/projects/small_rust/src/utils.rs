
pub fn format_currency(amount: f64, currency: &str) -> String {
    format!("{}{:.2}", currency, amount)
}

pub fn calculate_tax(amount: f64, rate: f64) -> f64 {
    amount * rate
}

pub fn validate_email(email: &str) -> bool {
    email.contains('@') && email.contains('.')
}

pub fn generate_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}
