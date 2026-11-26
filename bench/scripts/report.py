#!/usr/bin/env python3
"""Генератор отчёта о замерах экономии токенов (раздел 14.1 ТЗ).

Читает CSV с результатами сравнения и генерирует отчёт в Markdown.
"""

import csv
import sys
from pathlib import Path


def load_results(csv_path: Path) -> list[dict]:
    results = []
    with open(csv_path, newline='', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            results.append(row)
    return results


def compute_savings(results: list[dict]) -> dict:
    """Вычисляет экономию токенов по задачам."""
    tasks = {}
    for r in results:
        task = r['task_name']
        if task not in tasks:
            tasks[task] = {'without_atlas': [], 'with_atlas': []}
        mode = r['mode']
        tokens_out = int(r['tokens_out']) if r['tokens_out'] else 0
        calls = int(r['calls']) if r['calls'] else 0
        time_ms = int(r['time_ms']) if r['time_ms'] else 0
        tasks[task][mode].append({
            'tokens_out': tokens_out,
            'calls': calls,
            'time_ms': time_ms,
            'correct': r['correct'] == '1',
        })
    return tasks


def generate_report(tasks: dict) -> str:
    lines = []
    lines.append("# Отчёт о замере экономии токенов Atlas")
    lines.append("")
    lines.append("## Методика")
    lines.append("- Сравнение выполнения задач с Atlas и без Atlas")
    lines.append("- Экономия токенов = (tokens без Atlas - tokens с Atlas) / tokens без Atlas")
    lines.append("")
    lines.append("## Результаты по задачам")
    lines.append("")
    lines.append("| Задача | Токены без Atlas | Токены с Atlas | Экономия | Вызовы без Atlas | Вызовы с Atlas |")
    lines.append("|--------|------------------|----------------|----------|------------------|----------------|")

    total_savings = []
    for task, modes in tasks.items():
        without = modes.get('without_atlas', [{}])[0]
        with_atlas = modes.get('with_atlas', [{}])[0]

        tokens_without = without.get('tokens_out', 0)
        tokens_with = with_atlas.get('tokens_out', 0)
        calls_without = without.get('calls', 0)
        calls_with = with_atlas.get('calls', 0)

        if tokens_without > 0:
            savings = (tokens_without - tokens_with) / tokens_without * 100
            total_savings.append(savings)
        else:
            savings = 0

        lines.append(
            f"| {task} | {tokens_without} | {tokens_with} | {savings:.1f}% | "
            f"{calls_without} | {calls_with} |"
        )

    lines.append("")
    if total_savings:
        avg_savings = sum(total_savings) / len(total_savings)
        lines.append(f"## Итоговая экономия токенов: {avg_savings:.1f}% (среднее по задачам)")
        lines.append(f"Целевое значение (раздел 1.2): не менее 40%")
        if avg_savings >= 40:
            lines.append("✅ Цель достигнута")
        else:
            lines.append("❌ Цель не достигнута — требуется оптимизация")

    return "\n".join(lines)


def main():
    if len(sys.argv) < 2:
        print(f"Использование: {sys.argv[0]} <путь_к_csv> [выходной_файл.md]")
        sys.exit(1)

    csv_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("bench/results/REPORT.md")

    results = load_results(csv_path)
    tasks = compute_savings(results)
    report = generate_report(tasks)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(report, encoding='utf-8')
    print(f"Отчёт сохранён в: {output_path}")


if __name__ == "__main__":
    main()