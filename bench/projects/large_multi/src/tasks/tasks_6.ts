/**
 * tasks module - file 6
 */

export enum TasksStatus {
    ACTIVE = "active",
    INACTIVE = "inactive",
    PENDING = "pending"
}

export interface TasksItem {
    id: string;
    name: string;
    status: TasksStatus;
    metadata?: Record<string, unknown>;
}

export class TasksService {
    private items: TasksItem[] = [];
    
    create(itemId: string, name: string): TasksItem {
        const item: TasksItem = { id: itemId, name, status: TasksStatus.PENDING };
        this.items.push(item);
        return item;
    }
    
    get(itemId: string): TasksItem | undefined {
        return this.items.find(item => item.id === itemId);
    }
    
    listAll(): TasksItem[] {
        return [...this.items];
    }
    
    updateStatus(itemId: string, status: TasksStatus): boolean {
        const item = this.get(itemId);
        if (item) {
            item.status = status;
            return true;
        }
        return false;
    }
}

export function processTasks(data: Record<string, unknown>): { processed: number } {
    const service = new TasksService();
    for (const [key, value] of Object.entries(data)) {
        service.create(key, String(value));
    }
    return { processed: service.listAll().length };
}
