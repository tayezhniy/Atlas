
import pytest
from unittest.mock import AsyncMock, MagicMock
from src.products.service import ProductsService

@pytest.fixture
def mock_session():
    return AsyncMock()

@pytest.mark.asyncio
async def test_products_service_exists(mock_session):
    service = ProductsService(mock_session)
    assert service is not None
