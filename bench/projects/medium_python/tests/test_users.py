
import pytest
from unittest.mock import AsyncMock, MagicMock
from src.users.service import UsersService

@pytest.fixture
def mock_session():
    return AsyncMock()

@pytest.mark.asyncio
async def test_users_service_exists(mock_session):
    service = UsersService(mock_session)
    assert service is not None
