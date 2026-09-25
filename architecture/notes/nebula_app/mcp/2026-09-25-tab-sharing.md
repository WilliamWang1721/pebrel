# Tab-bound MCP sharing and local consent

## Status

Proposed for maintainer review with the implementation.

## Context

A cloud MCP client needs to operate a visible, persistent terminal without a
second local coding harness. Existing terminal state, shell submission barriers
and notifications already have owners; exporting the whole Runtime API would
expose unrelated panes and invisible execution paths.

## Evidence

- `nebula_app/src/gpui_shell/terminal/view/runtime.rs` owns real PTY submission,
  scrollback reads and shell-derived run results.
- `nebula_app/src/gpui_shell/workspace/notifications.rs` owns notification routing.
- `nebula_app/src/runtime_api.rs` already defines validated keys and inputs.
- OpenAI's `tunnel-client/docs/configuration.md` documents outbound tunnel binding
  through MCP_SERVER_URL, MCP_EXTRA_HEADERS and CONTROL_PLANE_API_KEY.

## Decision

The Advanced setting owns one loopback HTTP listener and its Tokio runtime.
Each live terminal owns a revocable share, a distinct route/credential and one
pending immutable write. rmcp owns protocol framing and SSE; the view owns
execution and consent. No remote method approves requests or selects panes.
Approval binds request identity and the terminal input epoch; actual writes
reuse existing Runtime methods on the owning UI thread.

Use the official externally installed tunnel-client instead of implementing its
protocol or packaging an updater. The helper gets credentials in its environment,
not command arguments or saved settings. A tunnel ID lease lasts until child
exit because two pollers on one ID can deliver work to different terminals.

Native notifications navigate to the tab. Their lifetime and CLI screen parsing
cannot grant MCP approval. The card is UI outside the terminal input surface.

## Rejected alternatives

- Exposing global Runtime credentials or pane.exec: breaks the visible terminal
  and target-selection contract.
- Treating paste/control keys as safe writes: both can execute arbitrary code.
- Intercepting shell command names: not a real sandbox and duplicates harnesses.
- A persistent approval queue: unnecessary for one terminal and makes consent
  harder to attribute. Concurrent writes receive an explicit busy result.
- Implementing Cloudflare or a tunnel wire client: unrelated protocol ownership
  and maintenance cost; the local MCP endpoint remains provider independent.

## Consequences

Three optional application dependencies add protocol/server infrastructure; no
terminal/settings-core dependency changes. Shares and helper keys are ephemeral.
MCP target scoping does not sandbox code already executing as the terminal user.
Stopping sharing cancels unsubmitted requests, not already running processes.
The helper must be installed, and remote platform eligibility remains external.

## Validation

Focused tests cover typed input validation, cross-share token rejection, browser
Origin rejection, initialization, revocation, single-writer admission, stale and
single-use approval, cancellation and exclusive tunnel IDs. They do not replace
native UI acceptance or authenticated ChatGPT end-to-end testing.

Manual acceptance must verify two simultaneous tabs, local approval before any
PTY input, paste/control-key approval, notification focus, disabling the master
setting, stopping helper processes, and existing scrollback visibility on each
supported OS. A successful compile does not establish these visual results.

## Supersedes

None.

## Revisit when

A real sandbox, restart recovery, additional remote transports or multi-client
terminal collaboration becomes an explicitly approved requirement.
