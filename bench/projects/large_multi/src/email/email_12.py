"""
email module - file 12
"""

from typing import List, Optional, Dict
from dataclasses import dataclass
from enum import Enum

class EmailStatus(Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"
    PENDING = "pending"

@dataclass
class EmailItem:
    id: str
    name: str
    status: EmailStatus = EmailStatus.PENDING
    metadata: Optional[Dict] = None

class EmailService:
    def __init__(self):
        self.items: List[EmailItem] = []
    
    def create(self, item_id: str, name: str) -> EmailItem:
        item = EmailItem(id=item_id, name=name)
        self.items.append(item)
        return item
    
    def get(self, item_id: str) -> Optional[EmailItem]:
        for item in self.items:
            if item.id == item_id:
                return item
        return None
    
    def list_all(self) -> List[EmailItem]:
        return self.items.copy()
    
    def update_status(self, item_id: str, status: EmailStatus) -> bool:
        item = self.get(item_id)
        if item:
            item.status = status
            return True
        return False

def processEmail(data: Dict[str, str]) -> Dict[str, int]:
    """Process email data"""
    service = EmailService()
    for key, value in data.items():
        service.create(key, value)
    return {"processed": len(service.items)}
