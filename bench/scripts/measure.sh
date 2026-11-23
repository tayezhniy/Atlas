#!/usr/bin/env bash
# Скрипт замера экономии токенов (раздел 14.1 ТЗ)
# Использование: ./measure.sh <путь_к_проекту>

set -euo pipefail

PROJECT_PATH="${1:-.}"
ATLAS_BIN="./target/debug/atlas-cli"

echo "=== Замер экономии токенов Atlas ==="
echo "Проект: $PROJECT_PATH"

# 1. Индексация проекта
echo "[1/4] Индексация проекта..."
$ATLAS_BIN index "$PROJECT_PATH"

# 2. Проверка состояния
echo "[2/4] Проверка состояния..."
$ATLAS_BIN doctor

# 3. Запуск MCP-сервера (в фоне)
echo "[3/4] Запуск MCP-сервера..."
$ATLAS_BIN mcp &
MCP_PID=$!
sleep 2

# 4. Остановка
echo "[4/4] Остановка MCP-сервера..."
kill $MCP_PID 2>/dev/null || true

echo "Готово. Подробности в docs/TZ.md (раздел 14)."