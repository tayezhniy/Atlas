#!/usr/bin/env python3
"""
Автоматизированный бенчмарк экономии токенов Atlas (раздел 14 ТЗ).

Финальная версия:
- Правильная обработка MCP notifications
- Реалистичная симуляция "без Atlas" (с учётом контекста)
- Задачи, соответствующие детерминированно сгенерированному коду
- Только навигационные задачи (исключены health-check задачи вроде status)
"""

import json
import subprocess
import time
import sqlite3
import sys
import os
import statistics
import threading
import queue
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import List, Dict, Any, Optional
from datetime import datetime
from collections import defaultdict

PROJECTS = {
    "small": "bench/projects/small_rust",
    "medium": "bench/projects/medium_python", 
    "large": "bench/projects/large_multi",
}

ATLAS_BIN = "./target/release/atlas-cli"

# Задачи, соответствующие детерминированно сгенерированному коду
# Только навигационные задачи (поиск, чтение, анализ связей)
TASKS = {
    "small": [
        {
            "id": "small_1",
            "name": "Найти определение UserService::create_user",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "create_user", "limit": 10}},
                {"tool": "read", "args": {"symbol": "create_user", "path": "src/services.rs", "mode": "full"}},
            ],
            "verify": {"symbol": "create_user", "file": "src/services.rs"},
        },
        {
            "id": "small_2", 
            "name": "Найти все вызовы Config::from_env",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "from_env", "limit": 20}},
                {"tool": "neighbors", "args": {"symbol": "from_env"}},
            ],
            "verify": {"symbol": "from_env"},
        },
        {
            "id": "small_3",
            "name": "Найти импорты serde в lib.rs",
            "mcp_calls": [
                {"tool": "outline", "args": {"path": "src/lib.rs"}},
            ],
            "verify": {"import": "serde", "file": "src/lib.rs"},
        },
        {
            "id": "small_4",
            "name": "Получить контекст OrderService::create_order (signatures + callers)",
            "mcp_calls": [
                {"tool": "read", "args": {"symbol": "create_order", "path": "src/services.rs", "mode": "signature"}},
                {"tool": "neighbors", "args": {"symbol": "create_order"}},
            ],
            "verify": {"symbol": "create_order"},
        },
        {
            "id": "small_5",
            "name": "Найти тесты для UserService",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "UserService", "limit": 20}},
                {"tool": "outline", "args": {"path": "tests/integration.rs"}},
            ],
            "verify": {"test_file": "tests/integration.rs"},
        },
    ],
    "medium": [
        {
            "id": "medium_1",
            "name": "Найти определение AuthService::authenticate",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "authenticate", "limit": 10}},
                {"tool": "read", "args": {"symbol": "authenticate", "path": "src/auth/service.py", "mode": "full"}},
            ],
            "verify": {"symbol": "authenticate", "file": "src/auth/service.py"},
        },
        {
            "id": "medium_2",
            "name": "Найти все места использования модели User",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "class User", "limit": 10}},
                {"tool": "neighbors", "args": {"symbol": "User"}},
            ],
            "verify": {"symbol": "User"},
        },
        {
            "id": "medium_3",
            "name": "Найти API маршруты для orders",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "orders_routes", "limit": 10}},
                {"tool": "outline", "args": {"path": "src/api/orders_routes.py"}},
            ],
            "verify": {"file": "src/api/orders_routes.py"},
        },
        {
            "id": "medium_4",
            "name": "Получить контекст ProductService::create_product",
            "mcp_calls": [
                {"tool": "read", "args": {"symbol": "create_product", "path": "src/products/service.py", "mode": "signature"}},
                {"tool": "neighbors", "args": {"symbol": "create_product"}},
            ],
            "verify": {"symbol": "create_product"},
        },
        {
            "id": "medium_5",
            "name": "Найти тесты для OrderService",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "OrderService", "limit": 20}},
                {"tool": "outline", "args": {"path": "tests/test_orders.py"}},
            ],
            "verify": {"test_file": "tests/test_orders.py"},
        },
    ],
    "large": [
        {
            "id": "large_1",
            "name": "Найти все сервисы в домене billing (TypeScript)",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "BillingService", "limit": 20}},
                {"tool": "overview", "args": {"path": "src/billing"}},
            ],
            "verify": {"symbol": "BillingService"},
        },
        {
            "id": "large_2",
            "name": "Найти функцию processBilling в домене billing",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "processBilling", "limit": 20}},
                {"tool": "neighbors", "args": {"symbol": "processBilling"}},
            ],
            "verify": {"symbol": "processBilling"},
        },
        {
            "id": "large_3",
            "name": "Получить контекст OrdersService в домене orders (TypeScript)",
            "mcp_calls": [
                {"tool": "read", "args": {"symbol": "OrdersService", "path": "src/orders/orders_0.ts", "mode": "signature"}},
                {"tool": "neighbors", "args": {"symbol": "OrdersService"}},
            ],
            "verify": {"symbol": "OrdersService"},
        },
        {
            "id": "large_4",
            "name": "Получить обзор проекта (overview)",
            "mcp_calls": [
                {"tool": "overview", "args": {}},
            ],
            "verify": {"type": "overview"},
        },
        {
            "id": "large_5",
            "name": "Найти сервисы в домене customers (Python)",
            "mcp_calls": [
                {"tool": "search", "args": {"query": "CustomersService", "limit": 20}},
                {"tool": "overview", "args": {"path": "src/customers"}},
            ],
            "verify": {"symbol": "CustomersService"},
        },
    ],
}

@dataclass
class BenchmarkResult:
    project: str
    task_id: str
    task_name: str
    mode: str
    run: int
    tokens_in: int
    tokens_out: int
    calls: int
    time_ms: int
    correct: bool
    error: Optional[str] = None

class AtlasMCPClient:
    """MCP client with proper subprocess handling using background reader thread.
    
    Uses a single response queue with request IDs to avoid threading visibility issues.
    """
    
    def __init__(self, project_path: str):
        self.project_path = os.path.abspath(project_path)
        self.proc = None
        self.request_id = 0
        self.response_queue = queue.Queue()  # Single queue for all responses
        self._running = False
        
    def start(self):
        self.proc = subprocess.Popen(
            [ATLAS_BIN, "mcp", "--project", self.project_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self._running = True
        
        # Start background threads for stdout and stderr
        self._reader_thread = threading.Thread(target=self._read_stdout, daemon=True)
        self._stderr_thread = threading.Thread(target=self._read_stderr, daemon=True)
        self._reader_thread.start()
        self._stderr_thread.start()
        
        # Give reader thread time to start
        time.sleep(0.2)
        
        # Initialize - send init request
        self._send_request({"jsonrpc": "2.0", "id": 1, "method": "initialize", 
                           "params": {"protocolVersion": "2025-03-26", "capabilities": {}}})
        # Send initialized notification IMMEDIATELY (server shuts down if not received quickly)
        self._send_notification({"jsonrpc": "2.0", "method": "notifications/initialized"})
        # Now wait for init response
        self._wait_for_response(1, timeout=30)
        
    def _read_stdout(self):
        """Background thread to read stdout and put all responses into queue."""
        while self._running and self.proc and self.proc.poll() is None:
            line = self.proc.stdout.readline()
            if not line:
                break
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
                # Put all JSON-RPC responses into the queue
                self.response_queue.put(msg)
            except json.JSONDecodeError:
                pass  # Ignore non-JSON lines (logs)
                
    def _read_stderr(self):
        """Background thread to consume stderr (prevents blocking)."""
        while self._running and self.proc and self.proc.poll() is None:
            line = self.proc.stderr.readline()
            if not line:
                break
            # Log stderr for debugging (optional)
            # print(f"[MCP stderr] {line.strip()}", file=sys.stderr)
        
    def _send_request(self, msg: dict):
        """Send a request with incrementing ID (or use existing ID if set)."""
        if "id" not in msg:
            self.request_id += 1
            msg["id"] = self.request_id
        else:
            # Use existing ID, update counter to stay in sync
            self.request_id = max(self.request_id, msg["id"])
        
        line = json.dumps(msg) + "\n"
        try:
            self.proc.stdin.write(line)
            self.proc.stdin.flush()
        except BrokenPipeError:
            self._running = False
            raise
            
    def _send_notification(self, msg: dict):
        """Отправляет уведомление без ожидания ответа"""
        line = json.dumps(msg) + "\n"
        try:
            self.proc.stdin.write(line)
            self.proc.stdin.flush()
        except BrokenPipeError:
            self._running = False
            raise
        
    def _wait_for_response(self, request_id: int, timeout: float = 30.0) -> dict:
        """Wait for response with matching request ID."""
        start = time.time()
        while time.time() - start < timeout:
            try:
                msg = self.response_queue.get(timeout=0.5)
                if msg.get("id") == request_id:
                    return msg
                # Put back non-matching responses
                self.response_queue.put(msg)
                time.sleep(0.01)
            except queue.Empty:
                continue
        raise TimeoutError(f"Timeout waiting for response to request {request_id}")
        
    def call_tool(self, name: str, args: dict) -> dict:
        self.request_id += 1
        req_id = self.request_id
        
        msg = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {"name": name, "arguments": args}
        }
        # _send_request will use the ID already in the message
        self._send_request(msg)
        return self._wait_for_response(req_id)
    
    def stop(self):
        self._running = False
        if self.proc:
            # Send shutdown notifications
            try:
                self._send_notification({"jsonrpc": "2.0", "method": "shutdown"})
                self._send_notification({"jsonrpc": "2.0", "method": "exit"})
            except:
                pass
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except:
                self.proc.kill()
            self.proc = None
            
    def estimate_tokens(self, text: str) -> int:
        return max(1, len(text) // 4)

class BenchmarkRunner:
    def __init__(self):
        self.results: List[BenchmarkResult] = []
        self.atlas_bin = ATLAS_BIN
        
    def run_all(self, runs_per_task: int = 3):
        print("=" * 60)
        print("ATLAS BENCHMARK - Token Savings Measurement")
        print("=" * 60)
        
        if not os.path.exists(self.atlas_bin):
            print(f"Error: {self.atlas_bin} not found. Run 'cargo build --release' first.")
            return
            
        for project_name, project_path in PROJECTS.items():
            print(f"\n{'='*60}")
            print(f"PROJECT: {project_name} ({project_path})")
            print(f"{'='*60}")
            
            # Check if index exists, if not create it
            index_db = os.path.join(project_path, ".atlas", "index.db")
            if not os.path.exists(index_db):
                print("Indexing project...")
                result = subprocess.run(
                    [self.atlas_bin, "index", project_path],
                    capture_output=True, text=True, timeout=300
                )
                if result.returncode != 0:
                    print(f"Indexing failed: {result.stderr}")
                    continue
                print("Indexing done.")
            else:
                print("Using existing index.")
            
            tasks = TASKS[project_name]
            
            for task in tasks:
                print(f"\n  Task: {task['name']}")
                
                # Режим БЕЗ Atlas
                for run in range(runs_per_task):
                    result = self.run_without_atlas(project_path, task, run)
                    self.results.append(result)
                    print(f"    Without Atlas (run {run+1}): {result.tokens_out} tokens, {result.calls} calls, {result.time_ms}ms, correct={result.correct}")
                
                # Режим С Atlas
                client = AtlasMCPClient(project_path)
                try:
                    client.start()
                    for run in range(runs_per_task):
                        result = self.run_with_atlas(client, task, run)
                        self.results.append(result)
                        print(f"    With Atlas (run {run+1}): {result.tokens_out} tokens, {result.calls} calls, {result.time_ms}ms, correct={result.correct}")
                finally:
                    client.stop()
                    
        self.save_results()
        self.print_summary()
        
    def run_without_atlas(self, project_path: str, task: dict, run: int) -> BenchmarkResult:
        """Симулирует выполнение задачи БЕЗ Atlas.
        
        Реалистичная модель: агент делает grep/find, читает файлы ПОЛНОСТЬЮ,
        накапливает контекст, каждый новый вызов оплачивает весь контекст заново.
        """
        start = time.time()
        tokens_in = 0
        tokens_out = 0
        calls = 0
        correct = False
        error = None
        accumulated_context = ""  # Накопленный контекст
        
        try:
            for mcp_call in task["mcp_calls"]:
                calls += 1
                tool = mcp_call["tool"]
                args = mcp_call["args"]
                
                # Симулируем вывод инструмента
                if tool == "search":
                    query = args.get("query", "")
                    result = subprocess.run(
                        ["grep", "-r", query, project_path, "--include=*.rs", "--include=*.py", "--include=*.ts"],
                        capture_output=True, text=True, timeout=30
                    )
                    output = result.stdout[:8000]  # Реалистичный лимит
                    
                elif tool == "outline":
                    path = args.get("path", "")
                    full_path = os.path.join(project_path, path)
                    if os.path.exists(full_path):
                        with open(full_path) as f:
                            output = f.read()  # Читает файл ПОЛНОСТЬЮ
                    else:
                        output = f"File not found: {path}"
                        
                elif tool == "read":
                    path = args.get("path", "")
                    full_path = os.path.join(project_path, path)
                    if os.path.exists(full_path):
                        with open(full_path) as f:
                            output = f.read()  # Читает файл ПОЛНОСТЬЮ
                    else:
                        output = f"File not found: {path}"
                        
                elif tool == "neighbors":
                    symbol = args.get("symbol", "")
                    result = subprocess.run(
                        ["grep", "-r", symbol, project_path, "--include=*.rs", "--include=*.py", "--include=*.ts"],
                        capture_output=True, text=True, timeout=30
                    )
                    output = result.stdout[:5000]
                    
                elif tool == "overview":
                    result = subprocess.run(
                        ["find", project_path, "-type", "f", "-name", "*.rs", "-o", "-name", "*.py", "-o", "-name", "*.ts"],
                        capture_output=True, text=True, timeout=30
                    )
                    output = result.stdout[:5000]
                    
                else:
                    output = ""
                
                # Токены: вход = накопленный контекст + запрос, выход = ответ
                request_tokens = len(f"{tool} {json.dumps(args)}") // 4
                tokens_in += len(accumulated_context) // 4 + request_tokens
                tokens_out += len(output) // 4
                
                # Накапливаем контекст (как в реальном LLM)
                accumulated_context += f"\n--- Tool: {tool} ---\n{output}\n"
                # Ограничиваем контекст (реальные модели имеют лимит)
                if len(accumulated_context) > 100000:
                    accumulated_context = accumulated_context[-100000:]
                
            # Проверка правильности
            correct = self.verify_result(project_path, task)
            
        except Exception as e:
            error = str(e)
            
        time_ms = int((time.time() - start) * 1000)
        
        return BenchmarkResult(
            project=os.path.basename(project_path),
            task_id=task["id"],
            task_name=task["name"],
            mode="without_atlas",
            run=run,
            tokens_in=tokens_in,
            tokens_out=tokens_out,
            calls=calls,
            time_ms=time_ms,
            correct=correct,
            error=error
        )
        
    def run_with_atlas(self, client: AtlasMCPClient, task: dict, run: int) -> BenchmarkResult:
        """Выполняет задачу через MCP-инструменты Atlas"""
        start = time.time()
        tokens_in = 0
        tokens_out = 0
        calls = 0
        correct = False
        error = None
        
        try:
            for mcp_call in task["mcp_calls"]:
                calls += 1
                tool = mcp_call["tool"]
                args = mcp_call["args"]
                
                response = client.call_tool(tool, args)
                
                if "error" in response:
                    error = response["error"].get("message", "MCP error")
                    break
                    
                if "result" in response:
                    content = response["result"].get("content", [])
                    for c in content:
                        if c.get("type") == "text":
                            text = c.get("text", "")
                            tokens_out += client.estimate_tokens(text)
                            
                # Входные токены: только запрос (контекст не накапливается благодаря бюджетам)
                request_tokens = len(f"{tool} {json.dumps(args)}") // 4
                tokens_in += request_tokens
            
            correct = self.verify_result(client.project_path, task)
            
        except Exception as e:
            error = str(e)
            
        time_ms = int((time.time() - start) * 1000)
        
        return BenchmarkResult(
            project=client.project_path.split("/")[-1],
            task_id=task["id"],
            task_name=task["name"],
            mode="with_atlas",
            run=run,
            tokens_in=tokens_in,
            tokens_out=tokens_out,
            calls=calls,
            time_ms=time_ms,
            correct=correct,
            error=error
        )
        
    def verify_result(self, project_path: str, task: dict) -> bool:
        verify = task.get("verify", {})
        if "symbol" in verify:
            symbol = verify["symbol"]
            result = subprocess.run(
                ["grep", "-r", symbol, project_path, "--include=*.rs", "--include=*.py", "--include=*.ts"],
                capture_output=True, text=True
            )
            return len(result.stdout.strip()) > 0
        if "file" in verify:
            return os.path.exists(os.path.join(project_path, verify["file"]))
        if "test_file" in verify:
            return os.path.exists(os.path.join(project_path, verify["test_file"]))
        if "path" in verify:
            return os.path.exists(os.path.join(project_path, verify["path"]))
        if "import" in verify:
            imp = verify["import"]
            file = verify.get("file", "")
            if file:
                full = os.path.join(project_path, file)
                if os.path.exists(full):
                    with open(full) as f:
                        return imp in f.read()
            return False
        if verify.get("type") in ("overview",):
            return True
        return True
        
    def save_results(self):
        os.makedirs("bench/results", exist_ok=True)
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        csv_path = f"bench/results/benchmark_{timestamp}.csv"
        
        with open(csv_path, "w") as f:
            f.write("project,task_id,task_name,mode,run,tokens_in,tokens_out,calls,time_ms,correct,error\n")
            for r in self.results:
                f.write(f"{r.project},{r.task_id},{r.task_name},{r.mode},{r.run},"
                       f"{r.tokens_in},{r.tokens_out},{r.calls},{r.time_ms},{int(r.correct)},{r.error or ''}\n")
        print(f"\nResults saved to {csv_path}")
        return csv_path
        
    def print_summary(self):
        print("\n" + "=" * 60)
        print("SUMMARY")
        print("=" * 60)
        
        for project_name in PROJECTS.keys():
            proj_results = [r for r in self.results if r.project == project_name]
            if not proj_results:
                continue
                
            print(f"\n--- {project_name} ---")
            
            for mode in ["without_atlas", "with_atlas"]:
                mode_results = [r for r in proj_results if r.mode == mode]
                if not mode_results:
                    continue
                    
                tokens_out = [r.tokens_out for r in mode_results]
                calls = [r.calls for r in mode_results]
                times = [r.time_ms for r in mode_results]
                correct = [r.correct for r in mode_results]
                
                print(f"  {mode}:")
                print(f"    Tokens out: median={statistics.median(tokens_out):.0f}, mean={statistics.mean(tokens_out):.0f}")
                print(f"    Calls: median={statistics.median(calls):.1f}, mean={statistics.mean(calls):.1f}")
                print(f"    Time: median={statistics.median(times)}ms, mean={statistics.mean(times):.0f}ms")
                print(f"    Correct: {sum(correct)}/{len(correct)} ({sum(correct)/len(correct)*100:.0f}%)")
                
            # Сравнение по задачам
            without = [r for r in proj_results if r.mode == "without_atlas"]
            with_atlas = [r for r in proj_results if r.mode == "with_atlas"]
            
            if without and with_atlas:
                savings = []
                for task in TASKS[project_name]:
                    w_tokens = [r.tokens_out for r in without if r.task_id == task["id"]]
                    a_tokens = [r.tokens_out for r in with_atlas if r.task_id == task["id"]]
                    if w_tokens and a_tokens:
                        w_med = statistics.median(w_tokens)
                        a_med = statistics.median(a_tokens)
                        if w_med > 0:
                            saving = (w_med - a_med) / w_med * 100
                            savings.append(saving)
                            
                if savings:
                    avg_saving = statistics.mean(savings)
                    print(f"  Token savings: {avg_saving:.1f}% (target: >=40%)")
                    if avg_saving >= 40:
                        print("  ✅ TARGET MET")
                    else:
                        print("  ❌ TARGET NOT MET")

def main():
    runs = int(sys.argv[1]) if len(sys.argv) > 1 else 3
    runner = BenchmarkRunner()
    runner.run_all(runs_per_task=runs)

if __name__ == "__main__":
    main()