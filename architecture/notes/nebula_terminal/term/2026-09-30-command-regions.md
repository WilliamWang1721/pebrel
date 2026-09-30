# Semantic command regions

## Status

Proposed.

## Context

An optional command-block UI needs the command, working directory and output
boundaries to agree with the terminal's parsed stream, scrollback and resizing.
An application-side copy of output would diverge from cursor rewrites and reflow.

## Evidence

- [StreamProcessor](../../../../nebula_terminal/src/event_loop.rs) already preserves
  OSC/VT wire order, including multiple shell events in one PTY read.
- [Prompt boundaries](../../../../nebula_terminal/src/term/prompt.rs) already own
  absolute prompt/input positions and suppress alternate-screen prompt marks.

## Decision

Keep optional semantic command regions beside existing prompt boundaries in the
terminal core. OSC 7 and 133 A/B/C/D update metadata at their actual parse positions;
the existing PowerShell command report supplies the original multiline input.
Output remains exclusively in the grid and is extracted only for a copy action.
Prune regions with their scrollback. Remap boundary anchors by logical line/cell
offset during resizing, including the primary buffer while a TUI is active.

The setting defaults off and applies to newly created integrated local sessions.
The shell shows cwd above the command; the view only owns selection, hover and
clipboard feedback. SSH keeps the existing terminal presentation. Re-input uses
the existing paste path, requires an empty shell input and never sends Enter;
multiline input additionally requires bracketed paste.

## Rejected alternatives

- Frontend-only OSC tracking: loses wire order when several commands share a read.
- Separate output strings/grids: duplicates scrollback and cursor/reflow authority.
- Clearing every boundary on resize: makes selected history blocks disappear.
- Immediate rerun: bypasses editable shell input and its paste confirmation.

## Consequences

Disabled sessions allocate no command-region collection or resize-anchor scan.
Enabled sessions retain only command/cwd metadata and boundary positions. Rendering
captures visible boundaries with the existing grid snapshot, without another lock
or a history text scan. Resize scans the retained primary grid only when enabled.
Unsupported/custom shell launches retain the existing unstructured terminal.

## Validation

[Core tests](../../../../nebula_terminal/src/term/prompt/tests.rs) exercise fragmented
streams, cwd/status, multiline/wide output, reflow, scrollback eviction and TUI
isolation. [Real control tests](../../../../nebula_app/src/gpui_shell/terminal/view/blocks/tests.rs)
exercise hitboxes, copy/expiry, keyboard navigation, drag selection and editable
re-input without execution. Shared settings tests cover opt-in, persistence and reset.

## Supersedes

None.

## Revisit when

Remote integration gains the same prompt layout and reliable input boundaries, or
a measured resize workload requires a cheaper boundary remapping algorithm.
