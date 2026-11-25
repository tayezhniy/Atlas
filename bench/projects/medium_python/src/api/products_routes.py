
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.ext.asyncio import AsyncSession
from src.database.session import get_session
from src.products.service import ProductsService
from pydantic import BaseModel
from typing import List, Optional

router = APIRouter(prefix="/products", tags=["products"])

@router.get("/")
async def list_products(session: AsyncSession = Depends(get_session)):
    service = ProductsService(session)
    return await service.list_products()

@router.get("/{item_id}")
async def get_products(item_id: int, session: AsyncSession = Depends(get_session)):
    service = ProductsService(session)
    item = await service.get_products(item_id)
    if not item:
        raise HTTPException(404, "Not found")
    return item
