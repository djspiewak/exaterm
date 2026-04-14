# GTK UI Glasscheck Plan

## Goal

Replace Exaterm's current ad hoc UI-test direction with a comprehensive Glasscheck-based GTK test suite that captures the current GTK UI and behavior in code. GTK is the source of truth. The resulting shared test crate and selector/scenario contract will later be reused unchanged to drive AppKit into parity, but this plan covers GTK work only.

## Non-Goals

- Do not implement any AppKit selector export, AppKit fixture mounting, or AppKit rendering fixes in this phase.
- Do not delete the existing AppKit-only visual tests in this phase.
- Do not persist GTK oracle PNGs or JSON snapshots. The GTK-derived contract should live in test code.
- Do not add terminal-emulator fidelity coverage in this phase. Focus on Exaterm-owned chrome, layout, text, and pointer-driven card/focus behavior.

## Current State To Preserve

- `crates/exaterm-gtk` is still binary-only and builds the GTK UI directly in `src/ui.rs`.
- `crates/exaterm-ui` already owns shared render data and presentation logic, including `CardRenderData`, `FocusRenderData`, `chrome_visibility`, layout helpers, and battle-card state derivation.
- The GTK UI has meaningful runtime branches for:
  - empty workspace vs battlefield
  - summarized vs sparse-shell cards
  - embedded-terminal vs scrollback-only battlefield
  - battlefield vs focus mode
  - nudge state and attention-bar visibility
- Glasscheck now provides a shared top-level facade:
  - `glasscheck::Harness`
  - `glasscheck::WindowHost`
  - `Scene`
  - `Selector`
  - anchored text assertions
  - region capture
  - `InputDriver` methods returning `Result<_, InputSynthesisError>`

## Deliverables

### 1. `exaterm-gtk` test-support API

Add a `lib` target to `crates/exaterm-gtk` and expose a public `test_support` module.

The GTK test-support API must:

- construct the real GTK UI, not a parallel fake renderer
- load the real CSS and icon configuration needed for layout and styling
- mount into `glasscheck::WindowHost`
- avoid:
  - beachhead connection
  - daemon startup
  - PTY spawning
  - summary/naming worker threads
  - timers whose behavior is unrelated to the tested scenario
- accept deterministic scenario inputs rather than reading environment or runtime state

The test-support API should expose a small mounting surface, conceptually:

```rust
pub fn mount_scenario(
    harness: &glasscheck::Harness,
    scenario: UiTestScenario,
) -> Result<MountedGtkUi, MountError>;
```

Where `MountedGtkUi` contains:

- `host: glasscheck::WindowHost`
- any native widget handles that are required for precise direct-input targeting during GTK-only test authoring

Keep the public API minimal. The shared tests should ultimately need only the `WindowHost`.

### 2. Shared selector/scenario contract in `exaterm-ui`

Add a small shared module in `crates/exaterm-ui` for GTK/AppKit UI test contracts. This module must not depend on GTK or AppKit.

It should define:

- `UiTestScenario`
- stable session keys for scenarios
- stable selector constructors/constants
- deterministic fixture data builders using shared `CardRenderData` / `FocusRenderData`-compatible inputs

Public selectors should be stable and human-readable. Use selectors, not scene-local ids, as the public test API.

Required selector coverage:

- `workspace.empty-state`
- `workspace.empty-state.title`
- `workspace.empty-state.body`
- `workspace.battlefield`
- `workspace.focus-panel`
- `focus.status-bar`
- `battlefield.card.<session>`
- `battlefield.card.<session>.title`
- `battlefield.card.<session>.status`
- `battlefield.card.<session>.headline`
- `battlefield.card.<session>.alert`
- `battlefield.card.<session>.nudge`
- `battlefield.card.<session>.attention-bar`
- `battlefield.card.<session>.scrollback`
- `battlefield.card.<session>.terminal-slot`
- `focus.card.<session>`
- `focus.card.<session>.title`
- `focus.card.<session>.status`
- `focus.card.<session>.headline`
- `focus.card.<session>.attention-pill`

Do not expose selectors for elements that are intentionally absent in some modes purely to mirror hidden implementation details. The contract should model user-visible UI.

### 3. GTK semantic scene export

Implement a GTK semantic scene source for the real mounted UI.

The scene source must:

- derive bounds from actual GTK widget allocations and mounted layout, not from reimplemented geometry guesses
- expose the shared selectors above
- attach appropriate roles and labels where meaningful
- include only selectors for visible user-facing elements in the current mode/state

Visibility rule: if GTK currently hides a UI element in a scenario, prefer omitting that selector from the scene so the shared tests can assert `assert_not_exists`.

Important GTK behavior that the scene source must capture:

- empty state visible only when there are no sessions
- sparse-shell cards hide title, status, headline, bars, and nudge row
- summarized cards show title/status/headline, and nudge row in battlefield mode
- bars visible only when summarized and not in focus mode
- focus mode hides battlefield embedded terminals
- focus mode shows focus panel and focus status bar
- battlefield shows scrollback instead of terminal slots when terminal embedding is not available

### 4. `crates/exaterm-ui-tests`

Create a new workspace crate `crates/exaterm-ui-tests`.

This crate is the single owner of the shared UI test suite.

Constraints:

- Add it to `workspace.members`
- Do not add it to `workspace.default-members`
- Depend on `glasscheck` with the current pinned git revision used during implementation
- Use target-specific feature activation:
  - Linux: `gtk`
  - macOS later: `appkit`

This crate should contain:

- one shared integration test file, `tests/ui_glasscheck.rs`
- any small shared assertion helpers needed by the suite
- no duplicated backend-specific assertion logic beyond scenario bootstrap

## Shared Test Structure

The shared test file should compile unchanged on Linux and later on macOS. Keep platform branching limited to bootstrap.

Allowed platform-specific code in the shared test file:

- creating the harness
- mounting the scenario through GTK test support

Everything after mounting should use only:

- `glasscheck::WindowHost`
- `Scene`
- `Selector`
- Glasscheck text assertions
- Glasscheck layout/semantic assertions
- region capture and programmatic pixel sampling
- `InputDriver` returning `Result`

## Scenarios To Implement In GTK Phase

Implement exactly these scenarios in the GTK phase.

### `EmptyWorkspace`

Purpose:

- lock down the no-session UI

Assertions:

- `workspace.empty-state` exists
- `workspace.empty-state.title` text renders as `No Live Sessions Yet`
- `workspace.empty-state.body` text renders
- `workspace.battlefield` does not exist
- `workspace.focus-panel` does not exist
- `focus.status-bar` does not exist

### `BattlefieldSingleSparse`

Use one unsummarized session in a window large enough that GTK embeds terminals in single-card mode.

Assertions:

- `battlefield.card.shell-1` exists
- `battlefield.card.shell-1.terminal-slot` exists
- `battlefield.card.shell-1.scrollback` does not exist
- `battlefield.card.shell-1.title` does not exist
- `battlefield.card.shell-1.status` does not exist
- `battlefield.card.shell-1.headline` does not exist
- `battlefield.card.shell-1.nudge` does not exist
- `battlefield.card.shell-1.attention-bar` does not exist
- card region is contained within battlefield root
- card chrome region renders non-flat background and visible border treatment

### `BattlefieldSingleSummarized`

Use one summarized session with headline, attention, and nudge state.

Assertions:

- card exists
- title/status/headline exist
- headline text renders
- nudge selector exists and its text renders
- attention-bar selector exists
- terminal slot exists in roomy single-card mode
- empty state does not exist
- focus panel does not exist

Programmatic visual assertions:

- top and bottom samples of card background differ in the gradient direction
- selected border region is visually brighter than unselected card edge
- attention bar has the expected number of filled segments
- empty segments differ from filled segments

### `BattlefieldFourMixed`

Use four sessions with mixed summary states and statuses in a window sized so GTK does not embed terminals.

Required fixture mix:

- one summarized active card
- one summarized blocked/attention-heavy card
- one sparse-shell card
- one summarized stopped or idle card

Assertions:

- exactly four card selectors exist
- cards occupy a 2x2 non-overlapping grid
- all cards expose scrollback selectors
- no card exposes terminal-slot selector
- per-card selector presence matches GTK rules:
  - sparse-shell card: title/status/headline/nudge/attention-bar absent
  - summarized cards: title/status present
  - headline present where GTK currently shows it
- exactly one selected visual state exists at a time

Programmatic visual assertions:

- transcript/scrollback band is darker than the surrounding card body
- transcript border remains subtle
- blocked/high-attention card’s bar fill is higher than the low-attention card’s bar fill

### `FocusSingleSummarized`

Start from `BattlefieldFourMixed`, then enter focus mode through direct pointer interaction.

Use direct pointer interaction for this test:

- resolve the selected card region
- call `host.input().click_rect_center(...)`
- fail hard if `InputSynthesisError` is returned in supported Linux/X11 execution

Assertions after focus entry:

- `workspace.focus-panel` exists
- `focus.card.<session>` exists
- `focus.card.<session>.title` text renders
- `focus.card.<session>.status` text renders
- `focus.card.<session>.headline` text renders
- `focus.status-bar` exists and text renders
- battlefield rail remains visible
- no `battlefield.card.<session>.terminal-slot` selectors exist while focus mode is active
- focus status bar region has distinct background treatment from the main focus card

### `FocusExit`

From `FocusSingleSummarized`, click the focused rail card again using direct pointer input.

Assertions:

- `workspace.focus-panel` no longer exists
- `focus.status-bar` no longer exists
- battlefield card grid is visible again

## Assertion Categories

The suite must comprehensively capture GTK using all four categories below.

### 1. Structural assertions

Use `Scene` and selector existence/count assertions for:

- element presence/absence
- card counts
- selection/focus mode structure
- scrollback vs terminal-slot mode switching

### 2. Layout assertions

Use Glasscheck layout assertions for:

- non-overlap of battlefield cards
- alignment of card header elements where visible
- containment of card regions within battlefield/focus roots
- expected ordering in grid and focus rail

### 3. Text assertions

Use anchored text assertions for all user-facing text that matters:

- empty state title/body
- card title
- status chip
- headline
- alert text when present
- nudge label
- focus headline
- focus status bar text

Do not replace these with plain string checks when rendered-text assertions are available.

### 4. Programmatic visual assertions

Use `capture_region(...)` and explicit pixel sampling in code for Exaterm-owned chrome:

- card background gradient
- selected border intensity
- scrollback band darkness and border subtlety
- attention-bar fill count and contrast
- focus status bar background contrast

Do not compare saved PNG baselines.

## Negative Assertion Discipline

For every major scenario, include negative assertions alongside positive ones.

Examples:

- empty state absent in non-empty workspace
- focus panel absent outside focus mode
- sparse-shell cards do not expose summary selectors
- terminal-slot absent in four-card battlefield
- scrollback absent when embedded terminal is active
- focus status bar absent after focus exit

The goal is to capture GTK precisely, not just loosely confirm that something renders.

## Calibration Requirement

Before considering the GTK phase complete, calibrate the suite on GTK itself.

For each assertion category, intentionally perturb GTK behavior and confirm the relevant test fails.

Minimum calibration:

- hide one required text element and confirm a text assertion fails
- shift one card/header region and confirm a layout assertion fails
- flatten or alter one sampled chrome treatment and confirm a visual assertion fails
- force one wrong visibility mode and confirm a positive or negative selector assertion fails

Do not mark the branch done until this calibration pass has been completed and the assertions have been tightened back to the intended thresholds.

## Verification Commands

Primary Linux verification command:

```bash
env GDK_BACKEND=x11 xvfb-run -a cargo test -p exaterm-ui-tests --test ui_glasscheck
```

Supplementary checks required before handoff:

```bash
cargo check -p exaterm-ui -p exaterm-gtk -p exaterm-ui-tests
```

If GTK initialization fails under Glasscheck:

- fail fast with a clear error
- do not silently skip the suite

## Done Criteria For The GTK Phase

This phase is complete only when all of the following are true:

- `exaterm-gtk` exposes a stable public test-support API
- `crates/exaterm-ui-tests` exists and contains the shared Glasscheck suite
- all listed scenarios are implemented on GTK
- the suite passes on Linux/X11 with the required command
- the suite includes both positive and negative assertions
- the suite has been calibrated against intentional GTK perturbations
- no AppKit work was required to complete the branch

## Explicit Next Step After This Plan

After the GTK phase is implemented on a Linux machine and committed to a branch, stop and return to the macOS machine. At that point, write `appkit-ui-glasscheck-plan.md` against the actual GTK branch shape rather than against assumptions.
