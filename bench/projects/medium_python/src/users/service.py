
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
