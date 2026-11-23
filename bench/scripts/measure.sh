#!/usr/bin/env bash
# Скрипт замера экономии токенов (раздел 14.1 ТЗ)
# Использование: ./measure.sh <путь_к_проекту> <задача>

set -euo pipefail

PROJECT_PATH="${1:-.}"
TASK="${2:-}"
ATLAS_BIN="./target/debug/atlas-cli"

if [ -z "$TASK" ]; then
    echo "Использование: $0 <путь_к_проекту> <задача>"
    echo "Доступные задачи смотрите в bench/tasks.md"
    exit 1
fi

echo "=== Замер экономии токенов Atlas ==="
echo "Проект: $PROJECT_PATH"
echo "Задача: $TASK"
echo ""

# 1. Индексация проекта
echo "[1/4] Индексация проекта..."
$ATLAS_BIN index "$PROJECT_PATH"

# 2. Проверка состояния индекса
echo "[2/4] Проверка состояния..."
$ATLAS_BIN doctor

# 3. Запуск MCP-сервера
echo "[3/4] Запуск MCP-сервера..."
$ATLAS_BIN mcp &
MCP_PID=$!
sleep 2

echo ""
echo "MCP-сервер запущен (PID: $MCP_PID)"
echo "Выполните задачу '$TASK' через MCP-клиент (Claude и т.д.)"
echo ""
echo "После завершения нажмите Enter для остановки сервера..."
read -r

# 4. Остановка
echo "[4/4] Остановка MCP-сервера..."
kill $MCP_PID 2>/dev/null || true

echo ""
echo "=== Замер завершен ==="
echo "Результаты смотрите в панели расхода токенов (UI) или в call_log БД."