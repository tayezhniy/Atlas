
use small_rust::{Config, services::UserService, models::User};

#[test]
fn test_user_creation() {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    let user = service.create_user("test@example.com".into(), "Test User".into());
    assert_eq!(user.email, "test@example.com");
    assert_eq!(user.name, "Test User");
}

#[test]
fn test_user_retrieval() {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    let user = service.create_user("test2@example.com".into(), "Test User 2".into());
    let retrieved = service.get_user(user.id).unwrap();
    assert_eq!(retrieved.email, user.email);
}

#[test]
fn test_user_update() {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    let user = service.create_user("test3@example.com".into(), "Test User 3".into());
    let updated = service.update_user(user.id, "Updated Name".into());
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().name, "Updated Name");
}

#[test]
fn test_user_deletion() {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    let user = service.create_user("test4@example.com".into(), "Test User 4".into());
    let deleted = service.delete_user(user.id);
    assert!(deleted);
    assert!(service.get_user(user.id).is_none());
}
