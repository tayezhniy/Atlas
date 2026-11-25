#!/usr/bin/env python3
"""Создание среднего Python проекта (~300 файлов)"""

import os
import random

root = "bench/projects/medium_python"
os.makedirs(os.path.join(root, "src"), exist_ok=True)
os.makedirs(os.path.join(root, "tests"), exist_ok=True)
os.makedirs(os.path.join(root, "docs"), exist_ok=True)

# Основные модули
modules = [
    "auth", "users", "products", "orders", "payments", 
    "inventory", "shipping", "notifications", "analytics",
    "api", "core", "utils", "config", "database", "cache"
]

# Создаём структуру пакетов
for mod in modules:
    os.makedirs(os.path.join(root, "src", mod), exist_ok=True)
    with open(os.path.join(root, "src", mod, "__init__.py"), "w") as f:
        f.write(f'"""Модуль {mod}"""\n')

# Корневые файлы
with open(os.path.join(root, "pyproject.toml"), "w") as f:
    f.write("""
[project]
name = "medium_python"
version = "0.1.0"
description = "Medium Python project for Atlas benchmarking"
requires-python = ">=3.10"
dependencies = [
    "fastapi>=0.100",
    "sqlalchemy>=2.0",
    "pydantic>=2.0",
    "redis>=4.5",
    "httpx>=0.24",
]

[project.optional-dependencies]
dev = ["pytest>=7.0", "pytest-asyncio>=0.21", "ruff>=0.1"]

[tool.pytest.ini_options]
asyncio_mode = "auto"
""")

# Главный модуль
with open(os.path.join(root, "src", "__init__.py"), "w") as f:
    f.write('"""Medium Python Project"""\n__version__ = "0.1.0"\n')

# Config
with open(os.path.join(root, "src", "config", "settings.py"), "w") as f:
    f.write("""
from pydantic_settings import BaseSettings
from typing import Optional

class Settings(BaseSettings):
    database_url: str = "sqlite:///app.db"
    redis_url: str = "redis://localhost:6379/0"
    api_host: str = "0.0.0.0"
    api_port: int = 8000
    secret_key: str = "dev-secret-change-in-production"
    debug: bool = True
    
    class Config:
        env_file = ".env"

settings = Settings()
""")

# Database
with open(os.path.join(root, "src", "database", "session.py"), "w") as f:
    f.write("""
from sqlalchemy.ext.asyncio import create_async_engine, AsyncSession, async_sessionmaker
from sqlalchemy.orm import declarative_base
from src.config.settings import settings

engine = create_async_engine(settings.database_url, echo=settings.debug)
async_session_maker = async_sessionmaker(engine, class_=AsyncSession, expire_on_commit=False)
Base = declarative_base()

async def get_session() -> AsyncSession:
    async with async_session_maker() as session:
        yield session

async def init_db():
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
""")

# Core models
with open(os.path.join(root, "src", "core", "models.py"), "w") as f:
    f.write("""
from sqlalchemy import Column, Integer, String, DateTime, ForeignKey, Enum as SQLEnum, Float
from sqlalchemy.orm import relationship
from src.database.session import Base
from datetime import datetime
import enum

class UserRole(str, enum.Enum):
    ADMIN = "admin"
    USER = "user"
    MANAGER = "manager"

class User(Base):
    __tablename__ = "users"
    id = Column(Integer, primary_key=True, index=True)
    email = Column(String, unique=True, index=True, nullable=False)
    hashed_password = Column(String, nullable=False)
    full_name = Column(String)
    role = Column(SQLEnum(UserRole), default=UserRole.USER)
    is_active = Column(Integer, default=1)
    created_at = Column(DateTime, default=datetime.utcnow)
    orders = relationship("Order", back_populates="user")

class Product(Base):
    __tablename__ = "products"
    id = Column(Integer, primary_key=True, index=True)
    name = Column(String, index=True, nullable=False)
    description = Column(String)
    price = Column(Float, nullable=False)
    stock = Column(Integer, default=0)
    category_id = Column(Integer, ForeignKey("categories.id"))
    created_at = Column(DateTime, default=datetime.utcnow)

class Category(Base):
    __tablename__ = "categories"
    id = Column(Integer, primary_key=True, index=True)
    name = Column(String, unique=True, index=True, nullable=False)
    products = relationship("Product", back_populates="category")

class Order(Base):
    __tablename__ = "orders"
    id = Column(Integer, primary_key=True, index=True)
    user_id = Column(Integer, ForeignKey("users.id"), nullable=False)
    total = Column(Float, nullable=False)
    status = Column(String, default="pending")
    created_at = Column(DateTime, default=datetime.utcnow)
    user = relationship("User", back_populates="orders")
    items = relationship("OrderItem", back_populates="order")

class OrderItem(Base):
    __tablename__ = "order_items"
    id = Column(Integer, primary_key=True, index=True)
    order_id = Column(Integer, ForeignKey("orders.id"), nullable=False)
    product_id = Column(Integer, ForeignKey("products.id"), nullable=False)
    quantity = Column(Integer, nullable=False)
    unit_price = Column(Float, nullable=False)
    order = relationship("Order", back_populates="items")
    product = relationship("Product")
""")

# Auth module
with open(os.path.join(root, "src", "auth", "service.py"), "w") as f:
    f.write("""
from src.core.models import User, UserRole
from src.database.session import get_session
from sqlalchemy import select
from typing import Optional
import hashlib

class AuthService:
    def __init__(self, session):
        self.session = session
    
    async def create_user(self, email: str, password: str, full_name: str = None, role: UserRole = UserRole.USER) -> User:
        hashed = hashlib.sha256(password.encode()).hexdigest()
        user = User(email=email, hashed_password=hashed, full_name=full_name, role=role)
        self.session.add(user)
        await self.session.commit()
        await self.session.refresh(user)
        return user
    
    async def authenticate(self, email: str, password: str) -> Optional[User]:
        hashed = hashlib.sha256(password.encode()).hexdigest()
        result = await self.session.execute(select(User).where(User.email == email, User.hashed_password == hashed))
        return result.scalar_one_or_none()
    
    async def get_user(self, user_id: int) -> Optional[User]:
        return await self.session.get(User, user_id)
    
    async def list_users(self, skip: int = 0, limit: int = 100):
        result = await self.session.execute(select(User).offset(skip).limit(limit))
        return result.scalars().all()
""")

# Users module
with open(os.path.join(root, "src", "users", "service.py"), "w") as f:
    f.write("""
from src.core.models import User
from src.database.session import get_session
from sqlalchemy import select
from typing import Optional, List

class UserService:
    def __init__(self, session):
        self.session = session
    
    async def get_profile(self, user_id: int) -> Optional[User]:
        return await self.session.get(User, user_id)
    
    async def update_profile(self, user_id: int, full_name: str = None, email: str = None) -> Optional[User]:
        user = await self.session.get(User, user_id)
        if user:
            if full_name:
                user.full_name = full_name
            if email:
                user.email = email
            await self.session.commit()
            await self.session.refresh(user)
        return user
    
    async def deactivate_user(self, user_id: int) -> bool:
        user = await self.session.get(User, user_id)
        if user:
            user.is_active = 0
            await self.session.commit()
            return True
        return False
    
    async def search_users(self, query: str, limit: int = 20) -> List[User]:
        result = await self.session.execute(
            select(User).where(User.full_name.ilike(f"%{query}%") | User.email.ilike(f"%{query}%")).limit(limit)
        )
        return result.scalars().all()
""")

# Products module
with open(os.path.join(root, "src", "products", "service.py"), "w") as f:
    f.write("""
from src.core.models import Product, Category
from src.database.session import get_session
from sqlalchemy import select
from typing import Optional, List

class ProductService:
    def __init__(self, session):
        self.session = session
    
    async def create_product(self, name: str, price: float, description: str = None, category_id: int = None, stock: int = 0) -> Product:
        product = Product(name=name, price=price, description=description, category_id=category_id, stock=stock)
        self.session.add(product)
        await self.session.commit()
        await self.session.refresh(product)
        return product
    
    async def get_product(self, product_id: int) -> Optional[Product]:
        return await self.session.get(Product, product_id)
    
    async def update_stock(self, product_id: int, quantity: int) -> bool:
        product = await self.session.get(Product, product_id)
        if product:
            product.stock = quantity
            await self.session.commit()
            return True
        return False
    
    async def list_products(self, category_id: int = None, skip: int = 0, limit: int = 50) -> List[Product]:
        query = select(Product)
        if category_id:
            query = query.where(Product.category_id == category_id)
        query = query.offset(skip).limit(limit)
        result = await self.session.execute(query)
        return result.scalars().all()
    
    async def search_products(self, query: str, limit: int = 20) -> List[Product]:
        result = await self.session.execute(
            select(Product).where(Product.name.ilike(f"%{query}%") | Product.description.ilike(f"%{query}%")).limit(limit)
        )
        return result.scalars().all()

class CategoryService:
    def __init__(self, session):
        self.session = session
    
    async def create_category(self, name: str) -> Category:
        category = Category(name=name)
        self.session.add(category)
        await self.session.commit()
        await self.session.refresh(category)
        return category
    
    async def list_categories(self) -> List[Category]:
        result = await self.session.execute(select(Category))
        return result.scalars().all()
""")

# Orders module
with open(os.path.join(root, "src", "orders", "service.py"), "w") as f:
    f.write("""
from src.core.models import Order, OrderItem, Product, User
from src.database.session import get_session
from sqlalchemy import select
from typing import Optional, List
from datetime import datetime

class OrderService:
    def __init__(self, session):
        self.session = session
    
    async def create_order(self, user_id: int, items: List[dict]) -> Optional[Order]:
        user = await self.session.get(User, user_id)
        if not user:
            return None
        
        total = 0.0
        order_items = []
        for item in items:
            product = await self.session.get(Product, item["product_id"])
            if not product or product.stock < item["quantity"]:
                return None
            total += product.price * item["quantity"]
            order_items.append({"product": product, "quantity": item["quantity"], "unit_price": product.price})
            product.stock -= item["quantity"]
        
        order = Order(user_id=user_id, total=total, status="pending")
        self.session.add(order)
        await self.session.flush()
        
        for oi in order_items:
            order_item = OrderItem(
                order_id=order.id,
                product_id=oi["product"].id,
                quantity=oi["quantity"],
                unit_price=oi["unit_price"]
            )
            self.session.add(order_item)
        
        await self.session.commit()
        await self.session.refresh(order)
        return order
    
    async def get_order(self, order_id: int) -> Optional[Order]:
        return await self.session.get(Order, order_id)
    
    async def list_user_orders(self, user_id: int, skip: int = 0, limit: int = 20) -> List[Order]:
        result = await self.session.execute(
            select(Order).where(Order.user_id == user_id).order_by(Order.created_at.desc()).offset(skip).limit(limit)
        )
        return result.scalars().all()
    
    async def update_status(self, order_id: int, status: str) -> bool:
        order = await self.session.get(Order, order_id)
        if order:
            order.status = status
            await self.session.commit()
            return True
        return False
""")

# API routes
for mod in ["auth", "users", "products", "orders"]:
    with open(os.path.join(root, "src", "api", f"{mod}_routes.py"), "w") as f:
        f.write(f"""
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.ext.asyncio import AsyncSession
from src.database.session import get_session
from src.{mod}.service import {mod.capitalize()}Service
from pydantic import BaseModel
from typing import List, Optional

router = APIRouter(prefix="/{mod}", tags=["{mod}"])

@router.get("/")
async def list_{mod}(session: AsyncSession = Depends(get_session)):
    service = {mod.capitalize()}Service(session)
    return await service.list_{mod if mod != "auth" else "users"}()

@router.get("/{{item_id}}")
async def get_{mod}(item_id: int, session: AsyncSession = Depends(get_session)):
    service = {mod.capitalize()}Service(session)
    item = await service.get_{mod if mod != "auth" else "user"}(item_id)
    if not item:
        raise HTTPException(404, "Not found")
    return item
""")

# Main app
with open(os.path.join(root, "src", "main.py"), "w") as f:
    f.write("""
from fastapi import FastAPI
from src.api import auth_routes, users_routes, products_routes, orders_routes
from src.database.session import init_db

app = FastAPI(title="Medium Python Project")

app.include_router(auth_routes.router)
app.include_router(users_routes.router)
app.include_router(products_routes.router)
app.include_router(orders_routes.router)

@app.on_event("startup")
async def startup():
    await init_db()

@app.get("/health")
async def health():
    return {"status": "ok"}
""")

# Tests
for mod in ["auth", "users", "products", "orders"]:
    with open(os.path.join(root, "tests", f"test_{mod}.py"), "w") as f:
        f.write(f"""
import pytest
from unittest.mock import AsyncMock, MagicMock
from src.{mod}.service import {mod.capitalize()}Service

@pytest.fixture
def mock_session():
    return AsyncMock()

@pytest.mark.asyncio
async def test_{mod}_service_exists(mock_session):
    service = {mod.capitalize()}Service(mock_session)
    assert service is not None
""")

# Docs
with open(os.path.join(root, "docs", "architecture.md"), "w") as f:
    f.write("""
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
""")

print(f"Created medium_python project at {root}")
