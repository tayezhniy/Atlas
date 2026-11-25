
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.ext.asyncio import AsyncSession
from src.database.session import get_session
from src.users.service import UsersService
from pydantic import BaseModel
from typing import List, Optional

router = APIRouter(prefix="/users", tags=["users"])

@router.get("/")
async def list_users(session: AsyncSession = Depends(get_session)):
    service = UsersService(session)
    return await service.list_users()

@router.get("/{item_id}")
async def get_users(item_id: int, session: AsyncSession = Depends(get_session)):
    service = UsersService(session)
    item = await service.get_users(item_id)
    if not item:
        raise HTTPException(404, "Not found")
    return item
