# AppKit UI Glasscheck Resumption Plan

## Goal

Bring the AppKit client into parity with the shared Glasscheck contract that now exists and passes on Linux.

This is a resumption document for the machine that will implement the macOS side. GTK is no longer the speculative source of truth; the source of truth is the code now present in:

- `crates/exaterm-ui/src/ui_test_contract.rs`
- `crates/exaterm-ui-tests/tests/ui_glasscheck.rs`
- `crates/exaterm-gtk/src/test_support.rs`

The AppKit implementation should make the shared suite compile and pass on macOS with minimal backend-specific branching outside scenario bootstrap.

## Current Linux Baseline

The Linux half is implemented and verified.

Passing verification:

```bash
cargo check -p exaterm-ui -p exaterm-gtk -p exaterm-ui-tests
env GDK_BACKEND=x11 xvfb-run -a cargo test -p exaterm-ui-tests --test ui_glasscheck -- --nocapture
```

The shared suite currently covers:

- `EmptyWorkspace`
- `BattlefieldSingleSparse`
- `BattlefieldSingleSummarized`
- `BattlefieldFourMixed`
- `FocusSingleSummarized`
- `FocusExit`

Important current contract details:

- Shared selectors live in `exaterm-ui::ui_test_contract::selectors`.
- There is no shared `focus.status-bar` selector.
- Focus-mode entry/exit is exercised by semantic click on the battlefield card selector for the selected session.
- The shared suite currently asserts:
  - structural presence/absence
  - text labels
  - non-overlap / containment
  - selected-state uniqueness
- The Linux suite no longer depends on image-capture assertions.
- The focused headline selector exists in the shared selector module, but it is not currently asserted by the shared test suite because it was not stable enough in GTK.

## Non-Goals

- Do not change the shared selector names unless the Linux contract is also updated deliberately.
- Do not add AppKit-only selectors to the shared test suite in this phase.
- Do not rewrite the shared scenarios to fit AppKit quirks.
- Do not add a separate AppKit-only duplicate of `ui_glasscheck.rs`.

## Deliverables

### 1. AppKit test-support mount API

Expose a public AppKit test-support mount surface analogous to GTK:

```rust
pub fn mount_scenario(
    harness: &glasscheck::Harness,
    scenario: UiTestScenario,
) -> Result<MountedAppKitUi, MountError>;
```

Requirements:

- mount the real AppKit UI, not a fake renderer
- use deterministic scenario fixtures from `exaterm-ui`
- avoid daemon/PTy/worker/timer side effects unrelated to the scenario
- return at least:
  - `host: glasscheck::WindowHost`

Keep any extra native handles private unless they are genuinely required for debugging or authoring.

### 2. AppKit semantic scene export

Implement a semantic scene source over the real mounted AppKit UI that exports the existing shared selector set.

Required selectors:

- `workspace.empty-state`
- `workspace.empty-state.title`
- `workspace.empty-state.body`
- `workspace.battlefield`
- `workspace.focus-panel`
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

Rules:

- derive bounds from live mounted layout
- emit selectors only for visible user-facing elements
- prefer omission over “hidden but present” semantics for shared assertions
- expose labels where text assertions need them
- export selected state on battlefield cards the same way the GTK scene does

### 3. Shared test bootstrap on macOS

Extend `crates/exaterm-ui-tests/tests/ui_glasscheck.rs` with the minimal `#[cfg(target_os = "macos")]` bootstrap needed to:

- create the Glasscheck harness
- call AppKit `mount_scenario`

Everything after mounting should stay shared.

### 4. Cargo wiring

Ensure the shared test crate enables the correct backend on macOS:

- `glasscheck` with AppKit support
- AppKit client crate dependency for test mounting

Do not move `exaterm-ui-tests` into `default-members`.

## Implementation Order

### 1. Match the shared contract before expanding it

Start by making the existing suite compile unchanged on macOS.

Do not start by adding more assertions.

### 2. Implement fixture-driven mount

Seed the AppKit client directly from `scenario_fixture(scenario)` data and force a deterministic initial selection/window size just as the GTK path does.

### 3. Export the battlefield selectors first

Get these passing first:

- `EmptyWorkspace`
- `BattlefieldSingleSparse`
- `BattlefieldSingleSummarized`
- `BattlefieldFourMixed`

Then add focus-mode semantics and interaction support for:

- `FocusSingleSummarized`
- `FocusExit`

### 4. Make semantic click work against the shared battlefield card selector

The shared focus tests currently click:

```rust
Selector::id_eq(selectors::battlefield_card(UiSessionKey::Shell2))
```

That means the AppKit host must resolve the clicked battlefield card to a real native target or equivalent semantic click path. Do not rely on changing the shared test to use a backend-specific selector.

## Scenario Expectations To Preserve

### `EmptyWorkspace`

- empty state exists
- battlefield absent
- focus panel absent

### `BattlefieldSingleSparse`

- one battlefield card
- terminal slot visible
- scrollback absent
- summary chrome absent

### `BattlefieldSingleSummarized`

- summarized battlefield chrome visible
- headline text includes the parser recovery fixture text
- nudge text includes `AUTONUDGE`
- terminal slot visible

### `BattlefieldFourMixed`

- four battlefield cards
- non-overlapping grid
- all cards in scrollback mode
- sparse shell lacks summary selectors
- exactly one selected card

### `FocusSingleSummarized`

- clicking the selected battlefield card enters focus mode
- focus panel exists
- focus card/title/status exist for `shell-2`
- battlefield rail remains visible
- battlefield terminal-slot selectors are absent while focused

### `FocusExit`

- clicking the selected battlefield card again exits focus mode
- focus panel disappears
- battlefield grid returns

## Known Contract Boundaries

These are intentional unless the Linux side is changed too:

- No shared `focus.status-bar`.
- No image-sampling requirements in the shared suite.
- No shared assertion currently requires `focus.card.<session>.headline`, even though the selector constructor exists.

If AppKit wants stronger coverage for those areas, add AppKit-only tests separately or first tighten the Linux contract and then update the shared suite.

## Suggested Verification On macOS

Use the macOS equivalent of:

```bash
cargo check -p exaterm-ui -p exaterm-ui-tests
cargo test -p exaterm-ui-tests --test ui_glasscheck -- --nocapture
```

Also run any AppKit-native Glasscheck contract/smoke tests already present in the local Glasscheck checkout before debugging Exaterm-specific failures.

## Recommended Debugging Sequence

1. Confirm the AppKit host can mount and expose a stable root scene.
2. Get `EmptyWorkspace` passing.
3. Get battlefield card selectors and labels passing.
4. Add selected-state export.
5. Add focus entry/exit semantic click support.
6. Only then investigate any remaining text/visibility mismatches.

## Files The macOS Implementer Should Read First

- `crates/exaterm-ui/src/ui_test_contract.rs`
- `crates/exaterm-ui-tests/tests/ui_glasscheck.rs`
- `crates/exaterm-gtk/src/test_support.rs`
- `crates/exaterm-gtk/src/ui.rs`

These files represent the implemented Linux contract and the closest backend analogue for the AppKit work.
