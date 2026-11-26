#!/usr/bin/env python3
"""Генератор отчёта о замерах экономии токенов (раздел 14.1 ТЗ).

Читает CSV с результатами сравнения и генерирует отчёт в Markdown.

Методика по ТЗ:
- Одна прогонка на задачу (run=0)
- Экономия считается только для задач, где ОБА режима дали верный ответ
- Итоговая экономия: медиана по задачам среднего и большого проектов
- Целевое значение: не менее 40%
"""

import csv
import sys
import statistics
from pathlib import Path


def load_results(csv_path: Path) -> list[dict]:
    results = []
    with open(csv_path, newline='', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            results.append(row)
    return results


def compute_savings(results: list[dict]) -> dict:
    """Вычисляет экономию токенов по задачам.
    
    Использует только первый прогон (run=0).
    Экономия засчитывается только если ОБА режима верны.
    """
    tasks = {}
    for r in results:
        if int(r['run']) != 0:
            continue  # Только первый прогон
        task = r['task_name']
        project = r['project']
        if task not in tasks:
            tasks[task] = {'project': project, 'without_atlas': None, 'with_atlas': None}
        mode = r['mode']
        tokens_out = int(r['tokens_out']) if r['tokens_out'] else 0
        calls = int(r['calls']) if r['calls'] else 0
        time_ms = int(r['time_ms']) if r['time_ms'] else 0
        correct = r['correct'] == '1'
        tasks[task][mode] = {
            'tokens_out': tokens_out,
            'calls': calls,
            'time_ms': time_ms,
            'correct': correct,
        }
    return tasks


def generate_report(tasks: dict) -> str:
    lines = []
    lines.append("# Отчёт о замере экономии токенов Atlas")
    lines.append("")
    lines.append("## Методика (раздел 14.1 ТЗ)")
    lines.append("- Одна прогонка на задачу (run=0)")
    lines.append("- Экономия токенов = (tokens без Atlas - tokens с Atlas) / tokens без Atlas")
    lines.append("- Засчитывается только если ОБА режима дали верный ответ (correct=1)")
    lines.append("- Итог: **медиана** экономии по задачам **среднего и большого** проектов")
    lines.append("- Целевое значение: не менее 40%")
    lines.append("")
    lines.append("## Результаты по задачам")
    lines.append("")
    lines.append("| Проект | Задача | Токены без Atlas | Токены с Atlas | Экономия | Вызовы без Atlas | Вызовы с Atlas | Оба верны |")
    lines.append("|--------|--------|------------------|----------------|----------|------------------|----------------|-----------|")

    savings_medium = []
    savings_large = []
    savings_all_correct = []

    for task_name, modes in tasks.items():
        project = modes['project']
        without = modes.get('without_atlas')
        with_atlas = modes.get('with_atlas')

        if not without or not with_atlas:
            continue

        tokens_without = without['tokens_out']
        tokens_with = with_atlas['tokens_out']
        calls_without = without['calls']
        calls_with = with_atlas['calls']
        both_correct = without['correct'] and with_atlas['correct']

        if tokens_without > 0 and both_correct:
            savings = (tokens_without - tokens_with) / tokens_without * 100
            savings_all_correct.append(savings)
            if project == "medium_python":
                savings_medium.append(savings)
            elif project == "large_multi":
                savings_large.append(savings)
            savings_str = f"{savings:.1f}%"
        else:
            savings_str = "—" if not both_correct else "0%"

        correct_str = "✅" if both_correct else "❌"
        lines.append(
            f"| {project} | {task_name} | {tokens_without} | {tokens_with} | {savings_str} | "
            f"{calls_without} | {calls_with} | {correct_str} |"
        )

    lines.append("")
    lines.append("## Итоговые значения экономии")
    lines.append("")

    if savings_medium:
        med_medium = statistics.median(savings_medium)
        lines.append(f"- **Средний проект (median)**: {med_medium:.1f}% по {len(savings_medium)} задачам")
    if savings_large:
        med_large = statistics.median(savings_large)
        lines.append(f"- **Большой проект (median)**: {med_large:.1f}% по {len(savings_large)} задачам")

    # Итог по ТЗ: медиана по среднему И большому проектам
    combined = savings_medium + savings_large
    if combined:
        final_median = statistics.median(combined)
        lines.append(f"- **Итог (median по medium+large)**: **{final_median:.1f}%**")
        lines.append(f"- Целевое значение (раздел 1.2): не менее 40%")
        if final_median >= 40:
            lines.append("✅ **Гейт пройден**")
        else:
            lines.append("❌ **Гейт не пройден** — требуется оптимизация")

    if savings_all_correct:
        lines.append("")
        lines.append(f"*Справка: среднее по всем верным задачам: {statistics.mean(savings_all_correct):.1f}% ({len(savings_all_correct)} задач)*")

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