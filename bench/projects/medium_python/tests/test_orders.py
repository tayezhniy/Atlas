
import pytest
from unittest.mock import AsyncMock, MagicMock
from src.orders.service import OrdersService

@pytest.fixture
def mock_session():
    return AsyncMock()

@pytest.mark.asyncio
async def test_orders_service_exists(mock_session):
    service = OrdersService(mock_session)
    assert service is not None
