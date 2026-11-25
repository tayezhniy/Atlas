
import pytest
from unittest.mock import AsyncMock, MagicMock
from src.auth.service import AuthService

@pytest.fixture
def mock_session():
    return AsyncMock()

@pytest.mark.asyncio
async def test_auth_service_exists(mock_session):
    service = AuthService(mock_session)
    assert service is not None
