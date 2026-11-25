
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use small_rust::{Config, services::UserService};

fn bench_user_creation(c: &mut Criterion) {
    c.bench_function("user_creation", |b| {
        b.iter(|| {
            let config = Config::from_env();
            let mut service = UserService::new(config);
            for i in 0..100 {
                service.create_user(
                    format!("user{}@example.com", i),
                    format!("User {}", i)
                );
            }
        })
    });
}

fn bench_user_lookup(c: &mut Criterion) {
    let config = Config::from_env();
    let mut service = UserService::new(config);
    for i in 0..1000 {
        service.create_user(format!("user{}@example.com", i), format!("User {}", i));
    }
    
    c.bench_function("user_lookup", |b| {
        b.iter(|| {
            service.get_user(black_box(500))
        })
    });
}

criterion_group!(benches, bench_user_creation, bench_user_lookup);
criterion_main!(benches);
