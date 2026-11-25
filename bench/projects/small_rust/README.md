
# Small Rust Project

Тестовый проект для бенчмарка Atlas (~50 файлов).

## Структура

- `src/lib.rs` - точка входа, реэкспорты
- `src/config.rs` - конфигурация
- `src/models.rs` - модели данных (User, Product, Order)
- `src/services.rs` - бизнес-логика (UserService, OrderService)
- `src/handlers.rs` - HTTP/веб обработчики
- `src/utils.rs` - утилиты
- `tests/integration.rs` - интеграционные тесты
- `benches/benchmark.rs` - бенчмарки

## Задачи для замера

1. Найти определение `UserService::create_user`
2. Найти все вызовы `Config::from_env`
3. Найти импорты `serde`
4. Оценить влияние изменения `OrderService::create_order`
5. Найти тесты для `UserService`
