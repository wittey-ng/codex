# Performance Testing & Benchmarks

This document outlines performance testing strategies and benchmarks for the Codex Web Server.

---

## Goals

1. **Latency**: HTTP overhead < 10% vs App Server
2. **Throughput**: Handle 100+ concurrent threads
3. **Memory**: Efficient memory usage under load
4. **SSE**: Support 1000+ concurrent SSE connections

---

## Benchmark Categories

### 1. HTTP Request Latency

Measure latency for individual HTTP requests.

**Tools**: `criterion`, `wrk`, `ab`

**Test Scenarios**:
- Thread creation (`POST /api/v2/threads`)
- Turn submission (`POST /api/v2/threads/:id/turns`)
- MCP server status (`GET /api/v2/mcp/servers`)
- Config read (`GET /api/v2/config`)

**Target**: p50 < 5ms, p95 < 20ms, p99 < 50ms

---

### 2. SSE Connection Overhead

Measure overhead of maintaining SSE connections.

**Metrics**:
- Connection establishment time
- Memory per connection
- Max concurrent connections
- Event delivery latency

**Target**: Support 1000+ connections with < 1GB memory

---

### 3. Throughput (Concurrent Threads)

Measure system throughput under concurrent load.

**Test Scenarios**:
- 10 concurrent threads, 100 turns each
- 100 concurrent threads, 10 turns each
- 1000 threads over 1 minute

**Target**: > 1000 turns/second

---

### 4. Approval Flow Latency

Measure end-to-end approval latency.

**Flow**:
```
Command execution → SSE approval request → Client POST → Op submission
```

**Metrics**:
- SSE event delivery time
- REST response processing time
- Total approval latency

**Target**: < 100ms end-to-end

---

## Benchmark Implementation

### Using Criterion (Rust)

Create `web-server/benches/http_latency.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use codex_web_server::*;
use tokio::runtime::Runtime;

fn bench_create_thread(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let state = rt.block_on(async {
        // Create test state
    });

    c.bench_function("create_thread", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Benchmark thread creation
            })
        })
    });
}

criterion_group!(benches, bench_create_thread);
criterion_main!(benches);
```

**Run**:
```bash
cargo bench -p codex-web-server
```

---

### Using wrk (HTTP Load Testing)

Test HTTP endpoint throughput:

```bash
# Install wrk
brew install wrk

# Benchmark thread creation (10 connections, 30 seconds)
wrk -t10 -c10 -d30s \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -s scripts/create_thread.lua \
  http://localhost:3000/api/v2/threads
```

**Lua Script** (`scripts/create_thread.lua`):
```lua
wrk.method = "POST"
wrk.body   = '{"model":"claude-sonnet-4-5"}'
wrk.headers["Content-Type"] = "application/json"
```

**Expected Output**:
```
Running 30s test @ http://localhost:3000/api/v2/threads
  10 threads and 10 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     5.23ms    2.15ms   50.00ms   89.12%
    Req/Sec   200.50     50.25   350.00     78.45%
  60000 requests in 30.00s, 15.00MB read
Requests/sec: 2000.00
Transfer/sec: 512.00KB
```

---

### Using Apache Bench (ab)

Simple HTTP benchmarking:

```bash
# 1000 requests, 100 concurrent
ab -n 1000 -c 100 \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -p create_thread.json \
  http://localhost:3000/api/v2/threads
```

**create_thread.json**:
```json
{"model":"claude-sonnet-4-5"}
```

---

## Memory Profiling

### Using Valgrind (Linux)

```bash
# Build with debug symbols
cargo build --bin codex-web-server

# Run with valgrind
valgrind --tool=massif \
  --massif-out-file=massif.out \
  ./target/debug/codex-web-server

# Analyze
ms_print massif.out
```

---

### Using Instruments (macOS)

```bash
# Build release binary
cargo build --bin codex-web-server --release

# Profile with Instruments
instruments -t "Time Profiler" ./target/release/codex-web-server
```

---

### Using heaptrack (Linux)

```bash
# Install heaptrack
sudo apt-get install heaptrack

# Profile
heaptrack ./target/debug/codex-web-server

# Analyze
heaptrack_gui heaptrack.codex-web-server.*.zst
```

---

## Comparative Benchmarks (App Server vs Web Server)

### Setup

Run both servers on same machine:

```bash
# App Server (port 3000)
codex-app-server --port 3000

# Web Server (port 3001)
codex-web-server --port 3001
```

### Test Script

```python
import asyncio
import aiohttp
import time
import json

async def bench_app_server():
    """Benchmark JSON-RPC App Server"""
    async with aiohttp.ClientSession() as session:
        start = time.time()

        for i in range(1000):
            payload = {
                "jsonrpc": "2.0",
                "id": i,
                "method": "thread/start",
                "params": {"model": "claude-sonnet-4-5"}
            }
            async with session.post(
                'http://localhost:3000/jsonrpc',
                json=payload
            ) as resp:
                await resp.json()

        elapsed = time.time() - start
        print(f"App Server: {1000/elapsed:.2f} req/s")

async def bench_web_server():
    """Benchmark REST Web Server"""
    async with aiohttp.ClientSession() as session:
        start = time.time()

        for i in range(1000):
            payload = {"model": "claude-sonnet-4-5"}
            async with session.post(
                'http://localhost:3001/api/v2/threads',
                json=payload,
                headers={"Authorization": "Bearer token"}
            ) as resp:
                await resp.json()

        elapsed = time.time() - start
        print(f"Web Server: {1000/elapsed:.2f} req/s")

asyncio.run(bench_app_server())
asyncio.run(bench_web_server())
```

**Run**:
```bash
python scripts/benchmark_comparison.py
```

**Expected**:
```
App Server: 2150.50 req/s
Web Server: 2000.00 req/s
Overhead: 7.0%
```

---

## SSE Connection Testing

### Test Script

```javascript
const EventSource = require('eventsource');

// Create 1000 SSE connections
const connections = [];
for (let i = 0; i < 1000; i++) {
  const es = new EventSource(
    `http://localhost:3000/api/v2/threads/thread-${i}/events`
  );

  es.onmessage = (event) => {
    console.log(`Connection ${i}: ${event.data}`);
  };

  es.onerror = (err) => {
    console.error(`Connection ${i} error:`, err);
  };

  connections.push(es);
}

// Monitor memory usage
setInterval(() => {
  const used = process.memoryUsage();
  console.log(`Memory: ${Math.round(used.heapUsed / 1024 / 1024)} MB`);
}, 5000);
```

**Run**:
```bash
node scripts/sse_load_test.js
```

**Monitor**:
```bash
# Server memory
ps aux | grep codex-web-server

# Network connections
netstat -an | grep 3000 | wc -l
```

---

## Performance Test Suite

### Automated Testing

Create `web-server/benches/performance_suite.rs`:

```rust
use criterion::{criterion_group, criterion_main, Criterion};

mod latency {
    pub fn bench_thread_creation(c: &mut Criterion) { /* ... */ }
    pub fn bench_turn_submission(c: &mut Criterion) { /* ... */ }
    pub fn bench_mcp_status(c: &mut Criterion) { /* ... */ }
}

mod throughput {
    pub fn bench_concurrent_threads(c: &mut Criterion) { /* ... */ }
    pub fn bench_turn_throughput(c: &mut Criterion) { /* ... */ }
}

mod sse {
    pub fn bench_sse_connection(c: &mut Criterion) { /* ... */ }
    pub fn bench_event_delivery(c: &mut Criterion) { /* ... */ }
}

criterion_group!(
    latency_benches,
    latency::bench_thread_creation,
    latency::bench_turn_submission,
    latency::bench_mcp_status
);

criterion_group!(
    throughput_benches,
    throughput::bench_concurrent_threads,
    throughput::bench_turn_throughput
);

criterion_group!(
    sse_benches,
    sse::bench_sse_connection,
    sse::bench_event_delivery
);

criterion_main!(latency_benches, throughput_benches, sse_benches);
```

**Run All Benchmarks**:
```bash
cargo bench -p codex-web-server
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
name: Performance Tests

on:
  push:
    branches: [main]
  pull_request:

jobs:
  benchmarks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run Benchmarks
        run: cargo bench -p codex-web-server

      - name: Upload Results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: target/criterion/

      - name: Compare with Baseline
        run: |
          # Compare with previous run
          # Fail if regression > 10%
```

---

## Performance Targets

### Latency Targets

| Operation | p50 | p95 | p99 |
|-----------|-----|-----|-----|
| Thread creation | < 5ms | < 20ms | < 50ms |
| Turn submission | < 10ms | < 50ms | < 100ms |
| MCP status | < 15ms | < 50ms | < 100ms |
| Config read | < 2ms | < 10ms | < 20ms |
| Approval response | < 5ms | < 20ms | < 50ms |

### Throughput Targets

| Metric | Target |
|--------|--------|
| Requests/second | > 2000 |
| Turns/second | > 1000 |
| SSE connections | > 1000 concurrent |

### Resource Targets

| Resource | Target |
|----------|--------|
| Memory per thread | < 10 MB |
| Memory per SSE connection | < 100 KB |
| CPU utilization (idle) | < 5% |
| CPU utilization (100 threads) | < 80% |

---

## Optimization Checklist

- [ ] Enable release optimizations (`--release`)
- [ ] Profile with `perf` / `Instruments`
- [ ] Optimize hot paths (identified via profiling)
- [ ] Use connection pooling for databases
- [ ] Enable HTTP/2 for multiplexing
- [ ] Implement request caching where appropriate
- [ ] Use async I/O for all operations
- [ ] Minimize allocations in hot paths
- [ ] Use `Arc` instead of `Clone` for shared state
- [ ] Enable LTO (Link-Time Optimization)

**Cargo.toml optimizations**:
```toml
[profile.release]
lto = "fat"
codegen-units = 1
opt-level = 3
```

---

## Monitoring in Production

### Metrics to Track

- Request latency (p50, p95, p99)
- Requests per second
- Error rate
- Active SSE connections
- Memory usage
- CPU usage
- Thread pool saturation

### Tools

- **Prometheus**: Metrics collection
- **Grafana**: Visualization
- **tokio-console**: Async runtime debugging
- **tracing**: Structured logging

---

## Next Steps

1. Implement criterion benchmarks (`benches/`)
2. Run comparative tests (App Server vs Web Server)
3. Profile with Instruments/perf
4. Optimize hot paths
5. Set up CI/CD performance regression tests
6. Document baseline performance metrics
