# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build          # Debug build
cargo build --release # Release build
cargo check          # Fast compile check (no linking)
cargo run            # Run the application
```

## Project Overview

Jita is a desktop application that generates and executes scripts from natural language input. It uses:
- **Slint** for UI (compiled via `build.rs`)
- **Anthropic Claude API** for script generation with structured output
- **SQLite** via `rusqlite` for persistence
- **tokio** for async runtime

## Architecture

### Entry Flow
```
main.rs
  ├── i18n::init()                    # Initialize i18n + Slint bundled translations
  ├── App::new()                       # Initialize DB, settings, LLM client
  └── ui::JitaWindow::new()            # Create Slint window
```

### Key Modules

| Module | Responsibility |
|--------|---------------|
| `src/app.rs` | Central hub holding `Arc<Mutex<AppState>>`, `Arc<Mutex<Database>>`, etc. |
| `src/llm.rs` | Anthropic API client. Uses `tool_use` to force structured JSON output. |
| `src/db.rs` | SQLite wrapper with `scripts`, `execution_records`, `uv_tool_cache` tables. |
| `src/task_manager.rs` | Spawns subprocesses, tracks running tasks via `Arc<Mutex<HashMap>>`. |
| `src/state.rs` | Window state machine: `Idle → Input → Generating → Reviewing/ParamInput → Executing`. |
| `src/i18n.rs` | Fluent-bundle based translations. Bundle is cached in `thread_local` and only rebuilt on locale change. |
| `src/execution.rs` | `ScriptExecutor` writes scripts to temp files and spawns `uv run`. |

### UI System

- `ui/main.slint` - Slint UI definition (compiled at build time)
- `src/ui/mod.rs` - `slint::include_modules!()` wrapper
- `build.rs` - Compiles Slint with `with_bundled_translations("lang")`

### Translation System (i18n)

**Slint UI**: `@tr("key")` in `.slint` → bundled `.po` files in `lang/<locale>/LC_MESSAGES/`

**Rust code**: `i18n::t("key")` or `i18n::t_args("key", &[("arg", val)])` → `.ftl` files in `locales/`

Locale is detected from `JITA_LANG` or `LANG` environment variable. Call `i18n::set_locale("en")` to change at runtime.

## Important Patterns

### Cross-thread UI Updates
```rust
// From tokio task → Slint UI
rt.spawn(async move {
    let result = do_async_work().await;
    slint::invoke_from_event_loop(move || {
        if let Some(w) = window.upgrade() {
            w.set_property(result.into());
        }
    });
});
```

### Weak Window References
Always use `Weak` for callbacks stored in async tasks. Slint components are not `Send`, so `Weak` prevents use-after-free.

### Async State Access
All state is wrapped in `Arc<Mutex<...>>`. Acquire lock, do minimal work, release. Don't hold locks during async I/O.

## Script Generation Flow

1. User enters natural language description
2. `App::generate_script()` collects uv tool summaries from DB
3. `LlmClient` builds system prompt with tool list + execution context
4. API call uses `tool_choice: { type: "tool", name: "generate_script" }` to force structured output
5. `generated_script_to_script()` converts `GeneratedScript` → `Script` model
6. Script is saved to DB and returned for review

## Data Files

- SQLite DB: `~/.local/share/jita/jita.db`
- Translations: `locales/*.ftl`, `lang/*/LC_MESSAGES/*.po`