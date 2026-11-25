
# Архитектура проекта

## Модули

- **auth** - аутентификация и авторизация
- **users** - управление профилями пользователей
- **products** - каталог товаров и категории
- **orders** - обработка заказов
- **payments** - платежи (заглушка)
- **inventory** - управление складом
- **shipping** - доставка
- **notifications** - уведомления
- **analytics** - аналитика
- **api** - HTTP маршруты
- **core** - общие модели и утилиты
- **config** - настройки
- **database** - работа с БД
- **cache** - кэширование

## Зависимости

```
auth -> users, database
users -> core, database
products -> core, database
orders -> users, products, database
api -> all modules
```

## Точки входа

- `src/main.py` - FastAPI приложение
- `src/api/*_routes.py` - маршруты
