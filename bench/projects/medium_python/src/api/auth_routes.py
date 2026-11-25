
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.ext.asyncio import AsyncSession
from src.database.session import get_session
from src.auth.service import AuthService
from pydantic import BaseModel
from typing import List, Optional

router = APIRouter(prefix="/auth", tags=["auth"])

@router.get("/")
async def list_auth(session: AsyncSession = Depends(get_session)):
    service = AuthService(session)
    return await service.list_users()

@router.get("/{item_id}")
async def get_auth(item_id: int, session: AsyncSession = Depends(get_session)):
    service = AuthService(session)
    item = await service.get_user(item_id)
    if not item:
        raise HTTPException(404, "Not found")
    return item
