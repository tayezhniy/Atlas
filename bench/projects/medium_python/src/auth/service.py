
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
