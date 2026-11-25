
from fastapi import FastAPI
from src.api import auth_routes, users_routes, products_routes, orders_routes
from src.database.session import init_db

app = FastAPI(title="Medium Python Project")

app.include_router(auth_routes.router)
app.include_router(users_routes.router)
app.include_router(products_routes.router)
app.include_router(orders_routes.router)

@app.on_event("startup")
async def startup():
    await init_db()

@app.get("/health")
async def health():
    return {"status": "ok"}
