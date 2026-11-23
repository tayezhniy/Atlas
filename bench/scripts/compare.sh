#!/usr/bin/env bash
# Сравнительный замер: с Atlas и без (раздел 14.1 ТЗ)
# Использование: ./compare.sh <путь_к_проекту> <файл_с_задачами>

set -euo pipefail

PROJECT_PATH="${1:-.}"
TASKS_FILE="${2:-bench/tasks.md}"
ATLAS_BIN="./target/debug/atlas-cli"

RESULTS_DIR="./bench/results"
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_FILE="$RESULTS_DIR/compare_$TIMESTAMP.csv"

echo "task,mode,tokens_in,tokens_out,calls,time_ms,correct" > "$RESULTS_FILE"

# Индексация проекта (для режима с Atlas)
echo "Индексация проекта для режима с Atlas..."
$ATLAS_BIN index "$PROJECT_PATH"

# Читаем задачи из файла (пропускаем заголовки и пустые строки)
TASKS=()
while IFS= read -r line; do
    # Извлекаем задачу из формата "N. Описание"
    if [[ "$line" =~ ^[0-9]+\.\ +\*?(.+)\*?$ ]]; then
        TASKS+=("${BASH_REMATCH[1]}")
    fi
done < "$TASKS_FILE"

echo "Найдено задач: ${#TASKS[@]}"

for task in "${TASKS[@]}"; do
    echo ""
    echo "=== Задача: $task ==="

    # Режим без Atlas (базовая линия: поиск и чтение файлов обычными средствами)
    echo "  Режим без Atlas..."
    START=$(date +%s%N)
    # Здесь выполняется задача без Atlas (grep, find, чтение файлов)
    # Результат записывается вручную или через скрипт клиента
    END=$(date +%s%N)
    TIME_NO_ATLAS=$(( (END - START) / 1000000 ))
    echo "    Время: ${TIME_NO_ATLAS}мс"

    # Режим с Atlas
    echo "  Режим с Atlas..."
    START=$(date +%s%N)
    # Здесь выполняется задача через MCP-инструменты Atlas
    END=$(date +%s%N)
    TIME_WITH_ATLAS=$(( (END - START) / 1000000 ))
    echo "    Время: ${TIME_WITH_ATLAS}мс"

    # Запись в CSV (заполняется вручную или через интеграцию с MCP-клиентом)
    echo "$task,without_atlas,0,0,0,$TIME_NO_ATLAS," >> "$RESULTS_FILE"
    echo "$task,with_atlas,0,0,0,$TIME_WITH_ATLAS," >> "$RESULTS_FILE"
done

echo ""
echo "Результаты сохранены в: $RESULTS_FILE"
echo "Заполните колонки tokens_in, tokens_out, calls, correct вручную или через интеграцию с MCP-клиентом."