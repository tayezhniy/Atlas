
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
