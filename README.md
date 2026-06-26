# oxidium-forge-core

The **Rust orchestrator daemon and REST API** for [Oxidium Forge](https://github.com/alex-kzr/oxidium-forge).

Runs as `forged` (daemon) and `forge` (CLI). Manages BPMN process definitions, executes instances with Zeebe-like stepping semantics, and exposes a REST API over `localhost`.

## Architecture

Cargo workspace with the following crates:

| Crate | Role |
|-------|------|
| `forge-model` | Domain types: i64 keys, enums, value model, error types |
| `forge-store` | SQLite (sqlx), migration runner → `schema_migrations`, repositories |
| `forge-bpmn` | BPMN parser (quick-xml), parsed model, validation, runtime graph compiler |
| `forge-engine` | Stepping engine, FEEL evaluator, jobs, manual tasks, events, incidents, history |
| `forge-api` | Axum REST API (DTOs, routes, handlers) |
| `forged` | Daemon binary: wires config + store + engine + API, lifecycle, graceful shutdown |
| `forge-cli` | Thin CLI (clap): REST client + `ensure_daemon_running()` |

## Quick Start

> **[planned]** — Available after Phase 1 (Daemon Foundation) lands.

```bash
# Build
cargo build --release

# Start daemon
./target/release/forge daemon start

# Check health
curl http://localhost:7700/health

# Deploy a BPMN diagram
./target/release/forge deploy path/to/process.bpmn

# Stop daemon
./target/release/forge daemon stop
```

## Documentation

All cross-cutting documentation lives in the umbrella repo:

| Section | Description |
|---------|-------------|
| [Architecture](https://github.com/alex-kzr/oxidium-forge/blob/main/docs/architecture/system-design.md) | System design and crate responsibilities |
| [REST API](https://github.com/alex-kzr/oxidium-forge/blob/main/docs/api/rest-api.md) | Full endpoint reference |
| [CLI Commands](https://github.com/alex-kzr/oxidium-forge/blob/main/docs/cli/commands.md) | `forge` command reference |
| [Data Model](https://github.com/alex-kzr/oxidium-forge/blob/main/docs/data-model/entities.md) | Database entities and schema |
| [ADRs](https://github.com/alex-kzr/oxidium-forge/blob/main/docs/decisions/adr-index.md) | Architecture Decision Records |
| [BPMN Support Matrix](https://github.com/alex-kzr/oxidium-forge/blob/main/docs/bpmn-elements/README.md) | Supported BPMN elements |

## License

This project is licensed under the MIT License. See [LICENSE.md](./LICENSE.md)
