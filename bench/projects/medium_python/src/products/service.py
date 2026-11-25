
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
