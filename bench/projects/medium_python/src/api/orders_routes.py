
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.ext.asyncio import AsyncSession
from src.database.session import get_session
from src.orders.service import OrdersService
from pydantic import BaseModel
from typing import List, Optional

router = APIRouter(prefix="/orders", tags=["orders"])

@router.get("/")
async def list_orders(session: AsyncSession = Depends(get_session)):
    service = OrdersService(session)
    return await service.list_orders()

@router.get("/{item_id}")
async def get_orders(item_id: int, session: AsyncSession = Depends(get_session)):
    service = OrdersService(session)
    item = await service.get_orders(item_id)
    if not item:
        raise HTTPException(404, "Not found")
    return item
