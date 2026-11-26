"""
roles module - file 18
"""

from typing import List, Optional, Dict
from dataclasses import dataclass
from enum import Enum

class RolesStatus(Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"
    PENDING = "pending"

@dataclass
class RolesItem:
    id: str
    name: str
    status: RolesStatus = RolesStatus.PENDING
    metadata: Optional[Dict] = None

class RolesService:
    def __init__(self):
        self.items: List[RolesItem] = []
    
    def create(self, item_id: str, name: str) -> RolesItem:
        item = RolesItem(id=item_id, name=name)
        self.items.append(item)
        return item
    
    def get(self, item_id: str) -> Optional[RolesItem]:
        for item in self.items:
            if item.id == item_id:
                return item
        return None
    
    def list_all(self) -> List[RolesItem]:
        return self.items.copy()
    
    def update_status(self, item_id: str, status: RolesStatus) -> bool:
        item = self.get(item_id)
        if item:
            item.status = status
            return True
        return False

def processRoles(data: Dict[str, str]) -> Dict[str, int]:
    """Process roles data"""
    service = RolesService()
    for key, value in data.items():
        service.create(key, value)
    return {"processed": len(service.items)}
