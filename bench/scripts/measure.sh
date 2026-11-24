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
$ATLAS_BIN doctor --project "$PROJECT_PATH"

# 3. Подключение клиента. MCP по stdio запускает сам клиент, поэтому скрипт сервер не стартует:
#    нужно добавить в конфигурацию клиента (Claude и др.) такую запись.
ABS_PROJECT="$(cd "$PROJECT_PATH" && pwd)"
ABS_BIN="$(cd "$(dirname "$ATLAS_BIN")" && pwd)/$(basename "$ATLAS_BIN")"
echo "[3/4] Подключите MCP-сервер к клиенту:"
cat <<JSON
{
  "mcpServers": {
    "atlas": { "command": "$ABS_BIN", "args": ["mcp", "--project", "$ABS_PROJECT"] }
  }
}
JSON
echo ""
echo "Выполните задачу '$TASK' в клиенте, затем нажмите Enter."
read -r

# 4. Итоги из журнала вызовов (call_log)
echo "[4/4] Вызовы и токены за сессию:"
python3 -I - "$ABS_PROJECT/.atlas/index.db" <<'PY'
import sqlite3, sys
c = sqlite3.connect(sys.argv[1])
rows = list(c.execute("SELECT tool, COUNT(*), SUM(tokens_out) FROM call_log GROUP BY tool ORDER BY 3 DESC"))
if not rows:
    print("  вызовов нет")
for tool, n, tokens in rows:
    print(f"  {tool:12} вызовов {n:3}  токенов ответа ~{tokens}")
print(f"  всего: вызовов {sum(r[1] for r in rows)}, токенов ответа ~{sum(r[2] for r in rows)}")
PY

echo ""
echo "=== Замер завершен ==="
