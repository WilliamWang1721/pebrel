# Prompt blocks share the terminal grid

## Status

Proposed with the experimental terminal-block feature; native UI acceptance remains required.

## Context

A command and its output should be selectable as one unit without replacing the
shell editor or buffering a second transcript. The existing OSC 133 integration
already owns prompt boundaries, but its row-only marks were cleared by resize.
Guessing boundaries from text would misidentify arbitrary command output.

## Evidence

- `nebula_terminal/src/event_loop.rs` delivers shell markers at their parser
  position rather than at the end of a PTY read.
- `nebula_terminal/src/term/prompt.rs` already retains ordered prompt marks and
  prunes them against the grid's scrollback floor.
- `nebula_terminal/src/term/blocks_tests.rs` exercises split control sequences,
  same-row prompts, Unicode, soft wrapping, resize, clipping and terminal modes.
- Height growth pulls history into the viewport and renumbers the absolute row
  origin. Preserving the old integer without accounting for this shift loses blocks.

## Decision

Keep prompt ownership in the terminal crate. Enrich the existing marks with their
column, derive half-open block ranges from consecutive marks and the live cursor,
and reuse the ordinary selection/extraction implementation. Block copying includes
the prompt, command and output, with no UI labels or added formatting.

Map marks through primary-grid reflow as logical-line offsets, including when that
grid is inactive behind an alternate screen. Drop anchors from partially evicted
logical lines instead of attaching old commands to unrelated retained text. This
mapping runs only on resize and stores coordinates, never output strings.

The GPUI adapter owns selection intent, hover, click-versus-drag and the existing
shared copy-feedback entity. Its visible-region vector is reused inside the existing
snapshot lock. Decorations and controls are overlays: they do not move cells, the
IME origin, the viewport, or the shell's input editor. Ordinary drag selection and
link activation remain separate. Alternate-screen, mouse-reporting and vi mode
retain their existing terminal interaction.

A single `terminal_blocks` setting is false when absent or invalid, uses the shared
persistence/reset contract, and applies to open panes without replacing sessions.
English and Chinese messages have typed IDs; other locales use the existing
explicit English fallback. Unsupported shells retain ordinary terminal behavior.

## Rejected alternatives

- A terminal instance or persistent text buffer per block duplicates state, changes
  application semantics, and grows memory with the transcript.
- Regex prompt detection cannot reliably distinguish prompts from user output.
- Clearing all marks on resize makes blocks disappear after ordinary window changes.
- Cell-level block metadata changes every cell's cost for an opt-in presentation.
- A separate command editor changes shell history, completion, IME and interactive
  application behavior; it is outside this feature's requirement.

## Consequences

No dependency, process, thread or new shell protocol is introduced. Prompt marks
use one extra machine word per retained mark; prompt tracking remains active even
when the UI experiment is off, as it also serves existing prompt navigation.
Resize mapping adds two linear grid walks and temporary storage proportional to
marks. Ordinary output does not create transcript copies. Visible-range lookup
uses the ordered marks; painting visits only visible blocks.

The experimental UI is not a claim of complete Warp feature parity. It preserves
the shell's editor rather than adding a separate rich-text input. Integration-free
shells, raw restored sessions lacking markers, and full-screen applications do not
receive fabricated blocks. A partially evicted block is unavailable for whole-block
copying rather than falsely advertised as complete.

## Validation

The focused block regressions and all 233 terminal unit tests passed using Rust
1.97.1 against the working production sources through an isolated offline manifest.
All 75 settings tests passed, including absent/invalid defaults, persistence and
reset. These are not substitutes for the locked complete workspace or native UI
checks. Added GPUI tests exercise real hitboxes, copy payload/feedback, keyboard
exit/navigation, drag selection, and live disabling. Native build/test results and
visual/DPI coverage must be recorded in the PR rather than inferred here.

## Supersedes

None.

## Revisit when

The shell protocol gains stable command IDs or explicit input/output subranges;
then replace coordinate-derived identity through a reviewed compatibility change.
Revisit the reflow mapping if grid storage changes. Remove or revise the overlay if
native accessibility, high-DPI or workload measurements demonstrate a regression.
