# Design: `/provider` Command for Model Provider Switching

## Background

Codex currently supports a `/model` command that lets users switch between
predefined model presets during a session. The configuration already supports
multiple `model_providers`, but choosing a different provider requires editing
`config.toml` and restarting Codex. A `/provider` command would allow users to
switch providers on the fly while keeping the existing conversation.

## Goals

- List all configured model providers (built‑in and user‑defined).
- Allow users to select a provider interactively via `/provider` similar to
  `/model`.
  - Apply the new provider without clearing the active session or conversation
    history.
  - Let each provider declare a `default_model`. When switching providers:
    - If the new provider supports the current model, keep it.
    - If the current model is unsupported and the provider defines
      `default_model`, switch to that model and notify the user.
    - If the provider lacks `default_model`, warn the user and require a
      manual model change.

## Non‑Goals

- Automatically mapping models between providers.

## High‑Level Design

### Protocol

- Extend `Op::OverrideTurnContext` with an optional
  `model_provider: Option<String>` field to request a provider change.
  - File: `codex-rs/protocol/src/protocol.rs`.
  - Update the TypeScript bindings by adding `#[ts(optional)]`.
- Update serialization/deserialization and regenerate the `ts-rs` output.

### Configuration

- Let each `ModelProviderInfo` declare an optional `default_model`.
  - File: `codex-rs/core/src/model_provider_info.rs`.
  - Built‑in providers should set this field (`openai` to the current CLI default; leave it unset for `oss`).
- Expose the field to users via `config.toml`:

```toml
[model_providers.my-provider]
name = "Acme"
base_url = "https://acme.example/v1"
default_model = "acme-best-1"
```

### Core (`codex-core`)

- In `submission_loop` (`codex-rs/core/src/codex.rs`) handle the new
  `model_provider` field:

  - Look up the provider in `Config.model_providers`; return an error if the id is unknown or credentials are missing.
  - Rebuild the `ModelClient` with the new provider while preserving the existing reasoning effort, approval policy, sandbox policy and cwd.
  - Determine whether the selected provider supports the current model. Use the provider's `/v1/models` endpoint when available or fall back to a static list; cache results to avoid repeated network calls.
  - If the model is unsupported and the provider defines a `default_model`, rebuild the client using that model and emit a warning response item.
  - If the model is unsupported and no `default_model` exists, emit a warning and leave the model unchanged so the user must run `/model`.
  - Extend `EnvironmentContext` (`codex-rs/core/src/environment_context.rs`) with optional `model_provider_id` and `model` fields, update its XML serialization, and emit it whenever the provider or model changes so session logs capture the switch.
  - Cache `/v1/models` results per provider and clear the cache when switching so lists from one provider do not leak into another. On network failure while fetching the list, treat the provider as not advertising any models and fall back to `default_model` logic.
  - Update `Config` with the new `model_provider_id`, `model_provider`, and effective `model` so subsequent status reports show the active provider and model. This update is in-memory only and does not persist to `config.toml`.

### TUI

- **SlashCommand**
  - Add `Provider` variant with command `/provider` and description
    "choose model provider".
  - Place it after `Model` in `slash_command.rs` so it appears near other
    configuration commands.
  - Include in `built_in_slash_commands()` and ensure
    `available_during_task` returns `false`.
- **Events**
  - Add `AppEvent::UpdateModelProvider(String)` and handle it in `app.rs` by
    calling `chat_widget.set_model_provider`.
- **ChatWidget**
  - Implement `open_provider_popup()` that builds a selection list from
    `config.model_providers`. Each item displays the provider's `name` and marks
    the current provider. Providers that require credentials but lack an API key
    should appear disabled with a short hint about missing auth.
  - On selection:
    - If the provider supports the current model, send
      `Op::OverrideTurnContext { model_provider: Some(id), model: None }`.
    - If unsupported and the provider defines `default_model`, include it in
      `model` so the core switches automatically.
    - Otherwise send the provider id only; the core will warn that the current
      model is invalid.
    - In all cases emit `AppEvent::UpdateModelProvider(id)`.
  - Add `set_model_provider(id)` to update
    `config.model_provider_id`/`config.model_provider` from
    `config.model_providers`.
- **Bottom Pane / Command Popup**
  - Ensure `/provider` appears in the slash command popup and is disabled while
    a task is running similar to `/model`.
  - Update slash popup ranking tests so `/provider` is suggested first for `/pr`.
  - Add help text in `history_cell.rs` alongside `/model` and `/approvals`.

### Status and Config Summary

- `create_config_summary_entries` already exposes the provider id; ensure any UI
  that renders status reflects the updated provider after switching.

### Session Logs

- Extend session logging to capture provider changes:
  - Update `tui/src/session_log.rs` so `log_inbound_app_event` records
    `AppEvent::UpdateModelProvider` with the provider id and name.
  - Ensure `EnvironmentContext` messages with updated provider/model are written
    to the JSONL log.

### Tests & Snapshots

- Add unit tests for `open_provider_popup` and command dispatch similar to the
  existing `/model` tests.
  - `codex-rs/tui/src/chatwidget/tests.rs` – popup selection and event
    dispatch.
  - `codex-rs/tui/src/bottom_pane/chat_composer.rs` and
    `command_popup.rs` – slash popup ranking and disabled-command behaviour.
- Update TUI snapshot tests and help text snapshots to include `/provider`.
- Add core tests in `codex-rs/core/tests` verifying that
  `OverrideTurnContext` with a new provider keeps conversation history intact,
  uses the requested provider for subsequent turns, and applies `default_model`
  fallback logic.
- Add session log tests ensuring provider switch events and updated
  `EnvironmentContext` entries are written to the JSONL log.

### Documentation

- Update user docs (`docs/config.md`, `docs/faq.md`, etc.) to mention the new
  `/provider` command, the `default_model` provider setting, and how switching
  providers handles unsupported models.

## Unsupported Model Handling

- When switching providers, Codex checks whether the current model is supported.
- If not and a `default_model` is configured, Codex switches to it and informs
  the user.
- If the provider lacks `default_model`, Codex warns that the current model is
  unsupported and requires the user to choose a new model manually.

## Implementation Steps

1. **Protocol:** add `model_provider` to `Op::OverrideTurnContext` and regenerate
   ts‑rs bindings.
2. **Configuration:** extend `ModelProviderInfo` with `default_model`, update
   built‑in providers, and document the field in `config.toml`.
3. **Core:** modify `submission_loop` to rebuild the client when the provider
   changes, perform model‑support checks, extend `EnvironmentContext` with
   provider/model fields, maintain per‑provider model caches, and apply
   `default_model` fallback.
4. **TUI:** implement the `/provider` slash command, popup UI, session log
   updates, event wiring, and config mutation helper.
5. **Tests:** add unit tests, snapshot tests, and core tests described above.
6. **Docs:** update end-user documentation and this design doc.
