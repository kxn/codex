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
- Update serialization/deserialization and ts-rs bindings accordingly.

### Core (`codex-core`)

- When handling `Op::OverrideTurnContext`, look up the requested provider from
  `Config.model_providers` using the provided id.
- Rebuild the `ModelClient` with the new provider while preserving:
  - current reasoning settings
  - approval/sandbox policies and other turn context values
- Determine whether the selected provider supports the current model.
  - If supported, keep the model.
  - If unsupported and the provider defines `default_model`, switch to it and
    emit a warning.
  - If unsupported and the provider lacks `default_model`, emit a warning and
    leave the model unchanged so the user can pick one manually.
- Update `Config` with the new `model_provider_id`, `model_provider`, and
  potentially updated `model` so that subsequent status reports show the active
  provider and model.

### TUI

- **SlashCommand**
  - Add `Provider` variant with command `/provider` and description
    "choose model provider".
  - Include in `built_in_slash_commands()` and the fuzzy command popup.
- **Events**
  - Add `AppEvent::UpdateModelProvider(String)` to propagate provider changes to
    the UI state.
- **ChatWidget**
  - Implement `open_provider_popup()` that builds a selection list from
    `config.model_providers`. Each item displays the provider's `name` and marks
    the current provider.
  - On selection:
    - If the provider supports the current model, send
      `Op::OverrideTurnContext { model_provider: Some(id), model: None }`.
    - If unsupported and the provider defines `default_model`, include it in
      `model` so the core switches automatically.
    - Otherwise send the provider id only; the core will warn that the current
      model is invalid.
    - In all cases emit `AppEvent::UpdateModelProvider(id)`.
  - Add `set_model_provider(id)` to update
    `config.model_provider_id`/`config.model_provider`.
- **Bottom Pane / Command Popup**
  - Ensure `/provider` appears in the slash command popup and is disabled while
    a task is running similar to `/model`.
  - Add help text in `history_cell.rs` if needed.

### Status and Config Summary

- `create_config_summary_entries` already exposes the provider id; ensure any UI
  that renders status reflects the updated provider after switching.

### Tests & Snapshots

- Add unit tests for `open_provider_popup` and command dispatch similar to the
  existing `/model` tests.
- Update TUI snapshot tests and help text snapshots to include `/provider`.
- Add core tests verifying that `OverrideTurnContext` with a new provider keeps
  conversation history intact and uses the requested provider for subsequent
  turns.

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

1. Extend protocol enums/structs with `model_provider` field.
2. Add `default_model` to provider config structs and parsing.
3. Update core `Codex` logic to rebuild `TurnContext` when provider changes and
   apply the unsupported-model logic above.
4. Wire up new TUI command, events, and selection popup.
5. Adjust tests and snapshots.
6. Update documentation.
