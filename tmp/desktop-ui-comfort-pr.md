## Result / 用户结果

Improve undersized Settings controls and toolbar buttons reported on macOS through the shared GPUI presentation path also used on Windows.

- Shared Settings action buttons and select triggers receive a 32 logical-pixel minimum height, growing with larger UI fonts; action buttons get 12px horizontal padding.
- Short segmented settings, including character spacing, use regular-size labels, matching height and a moderate 6px corner radius rather than a thin small-label capsule.
- Settings, sidebar toggles and command-manager toolbar controls use 18px glyphs inside actual 32×32px hover/click surfaces. Sidebar title-bar gaps are explicitly 8px rather than font-dependent rem spacing.
- Settings help/reset targets increase from 22/20px to 32px.

Latest main already has 34px font +/- buttons and 34px Settings navigation rows; these are retained. Terminal font, cell geometry, configuration, native window buttons and interaction callbacks are unchanged.

## Design / 设计边界

The pinned component Root makes the UI font size the rem base. Medium Button uses `h_8` / `size_8` (2rem), so the default 14px UI font produces a 28px button even where the workspace comment assumes 32px. Short-choice groups additionally force the Small variant. This patch separates control geometry from small UI text without globally changing rem or terminal density.

Responsibility remains in the existing widgets, Settings and workspace-toolbar presentation modules. Two small shared helpers reuse the component library. No dependency, setting, framework, data format, threading or lifetime change. Select trigger styling does not change popup/search size semantics. The existing controls continue to supply interaction states and callbacks.

References:
- [Apple HIG: Buttons](https://developer.apple.com/design/human-interface-guidelines/buttons): recognizable controls, surrounding space, consistent sizing and press feedback.
- [Apple HIG: Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars): consistent toolbar presentation.
- [Microsoft: Content layout and spacing](https://learn.microsoft.com/en-us/windows/apps/design/style/spacing): contextual 8 effective-pixel button gaps.

The 32px/18px choices follow this project's existing desktop guidance, not an Apple-mandated universal minimum or a touch-sized redesign. The shared Windows path benefits from the same change, but Windows visual acceptance has not yet been performed.

## Evidence / 验证依据

One independent commit (`0aaaeb3b06b417ee3aab7e56b68dd6f5d232f2dc`) directly on upstream main `9dc058d12765893553d5fc7a2c37c870c96168b0`: 8 source/test files, +116/-43. The temporary preparation workflow/scripts live on a separate fork-only automation branch and are absent from this PR.

Passed with Rust 1.97.1 in the isolated preparation runner:
- `cargo fmt --all -- --check`
- `git diff --check`
- `python3 scripts/check_architecture.py --base 9dc058d12765893553d5fc7a2c37c870c96168b0`

Added one GPUI regression test: render shared action/toolbar buttons at 10px, 14px and 24px UI fonts, inspect their actual layout bounds, click inside padding rather than on the glyph, and confirm disabled padding does not activate. Pre-fix rem-based action buttons fail the small-font height assertion. **The test is authored but not yet executed; native compilation/tests and the gpui-test-support run remain pending.**

**Outstanding UI acceptance:** actual macOS before/after screenshots, light/dark themes, long translations and narrow windows, keyboard-focus traversal, Retina / mixed-DPI behavior, and Windows visual review. No compile or virtual-window result is claimed as native visual evidence. This is initially a Draft PR for that reason.

No terminal rendering hot-path change; no benchmark claim.

## Required Review / 必须确认

- [x] The implementation keeps the presentation-layer boundary from CONTRIBUTING, architecture and project constraints; outstanding UI acceptance is listed explicitly.
- [x] No duplicate behavior authority was added.
- [x] The architecture checker passes against the actual upstream base; no budgets were inflated.
- [ ] Execute the added GPUI regression and native compile/test checks.
- [ ] Complete macOS/Windows visual, keyboard, long-label and DPI acceptance.
- New messages / governance changes: not applicable; neither is changed.

Checkboxes do not replace CI or maintainer review.
