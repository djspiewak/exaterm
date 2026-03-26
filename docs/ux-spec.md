# Exaterm UX Specification

## Product Definition

Exaterm is a Linux desktop app for supervising multiple terminal-native coding agents at once.

The product is:

- grid-first, detail-on-demand
- a supervisor layer around unmodified terminal apps
- centered on agent runs and sessions, not generic shell panes

The product is not:

- a replacement for Codex, Claude Code, or other terminal-native agents
- a custom agent shell that hides native terminal UX
- a dashboard-heavy terminal multiplexer

Core design rule: the native TUI stays primary.

## Primary User Posture

The default working posture is a grid of live agent sessions on screen. Each tile mostly shows the real terminal surface for that session. The operator scans the grid, identifies which agent needs attention, opens a probe when deeper inspection is needed, then dismisses the probe and returns to the clean grid.

The operator should be able to answer these questions at a glance:

- Which agent is currently active?
- Which agent is blocked?
- Which agent failed?
- Which agent needs intervention first?

## Design Principles

- Grid-first, detail-on-demand.
- Native TUI stays primary.
- Probes are temporary inspection surfaces, not a second permanent layout.
- Tile chrome stays light.
- Observability should help prioritization before diagnosis.
- Features should not assume deep agent-specific hooks.

## Object Model

### Session

A session is the primary object in the system. A session represents one supervised terminal-native agent run.

Each session has:

- a terminal surface
- a stable session identity
- a launch configuration
- runtime state
- process tree metadata
- recent event history
- optional probes attached to it

### Tile

A tile is the grid representation of one session.

Each tile contains:

- the live terminal view
- light session chrome
- minimal status signals
- entry points for opening probes or switching lens

### Probe

A probe is an attachable inspection surface associated with one session and one lens.

A probe:

- belongs to a source tile
- floats over the grid
- may partially obscure nearby tiles
- can be repositioned
- can be resized
- can be dismissed quickly
- exists in transient or pinned form

### Lens

A lens defines what a probe shows.

Initial supported lenses:

- stdout
- process tree
- events

Optional tile lens switching may also expose:

- TUI
- stdout
- process tree
- events

## V1 Observability Boundaries

Exaterm v1 operates as a supervisor around unmodified terminal apps.

Available without deep integration:

- PTY/session capture
- terminal stream capture
- process tree tracking via `/proc`
- operator controls
- session event derivation from process and stream observations

Available with shallow adapters:

- semantic hints inferred from logs or known output patterns

Not assumed in v1:

- internal agent turn state
- tool call state
- model thinking state
- perfect attribution of stdout to arbitrary subprocesses

Important constraint: v1 should treat captured output as session output by default. A "main process stdout" view is allowed only when launch control or wrapping makes that attribution reliable enough.

## Information Architecture

### Main Screen

The main screen is a resizable grid of session tiles with floating probes layered above it.

Main screen regions:

- grid canvas
- optional top app bar for workspace-level controls
- optional shared detail area only if it does not compete with the grid
- floating probe layer

The grid is always the dominant region.

### Tile Anatomy

Each tile has five parts:

1. Header
2. Terminal body
3. Lightweight status strip
4. Focus/selection affordance
5. Probe anchor affordance

#### Header

The header is one line tall in the normal state.

Header contents:

- session name
- agent label or launch profile
- current high-level status
- optional short dominant-process label
- minimal probe controls

The header should never expand into multiline diagnostics in the default view.

#### Terminal Body

The terminal body is the primary surface and occupies most of the tile.

Requirements:

- show the real embedded terminal
- preserve the agent's native keyboard-driven UX
- avoid overlays that permanently cover meaningful terminal area
- allow focus to transfer cleanly into the terminal surface

#### Lightweight Status Strip

The status strip gives glanceable signals without becoming a dashboard.

Candidate fields:

- status chip
- elapsed runtime
- recent event summary
- last operator action

At most one short recent-event line should be visible by default.

#### Selection and Probe Affordances

Each tile must clearly show:

- whether it is selected
- whether it has one or more open probes
- which probe, if any, currently has focus

Probe affordances should remain compact until hover or selection.

### Tile Status Model

Session status is intentionally coarse and operator-facing.

Primary statuses:

- Running
- Waiting
- Attention
- Blocked
- Failed
- Complete
- Detached

Definitions:

- `Running`: terminal activity or process activity indicates active work
- `Waiting`: session is live but currently quiet or awaiting external progress
- `Attention`: heuristic signal suggests operator review is useful soon
- `Blocked`: likely waiting on user input, permission, or a failed dependency
- `Failed`: the session or main launched activity exited unexpectedly or entered a clear error condition
- `Complete`: the intended run appears finished
- `Detached`: session process state is unavailable or the terminal backend disconnected

Status assignment should be explainable from observable evidence. Avoid false precision.

### Probe Anatomy

Every probe has:

- title bar
- source-tile indicator
- lens selector or lens label
- probe content area
- close control
- optional pin state
- optional follow/freeze control

#### Title Bar

The title bar should include:

- source session name
- current lens
- pinned or transient state
- close button

#### Source-Tile Indicator

The relationship between probe and source tile must remain obvious.

Possible mechanisms:

- shared accent color
- session badge repeated on both tile and probe
- subtle anchor line or directional notch
- highlight ring on source tile while probe is focused

At least two of these should be used so the linkage survives visual clutter.

#### Probe Content Area

Probe content is optimized for inspection, not control density.

Each lens should emphasize:

- readability
- live updating
- quick orientation
- low-friction dismissal

## Probe Types

### Stdout Probe

Purpose: inspect live session output or controlled main-process output without switching the tile away from the terminal.

Contents:

- live scrolling text view
- timestamp toggle
- auto-follow toggle
- pause/freeze control
- simple find/filter

Behavior:

- default is follow-on
- operator can freeze scroll without stopping capture
- if "main process stdout" cannot be guaranteed, label the content as session output

### Process Tree Probe

Purpose: inspect the current subprocess structure and identify runaway, blocked, or unexpected child processes.

Contents:

- parent-child process tree
- pid
- command
- state
- start time or duration
- resource summary if cheap to obtain

Behavior:

- tree updates live
- recent process births and exits are highlighted briefly
- selecting a process may reveal a small inline detail row, but v1 should avoid turning this into a full system monitor

### Events Probe

Purpose: show a compact timeline of meaningful session events.

Candidate event classes:

- session launched
- process spawned
- process exited
- quiet period detected
- likely prompt-for-input detected
- failure signature detected
- probe opened or pinned
- operator intervened

Behavior:

- newest events appear at top by default
- event items should be short and scannable
- each event should make clear whether it is derived from observation or known with certainty

## Probe Modes

### Peek Probe

A peek probe is transient.

Properties:

- opens quickly near the source tile
- intended for short inspection
- closes with `Esc`, close button, or focus-dismiss behavior if enabled
- should not alter the overall layout

### Pinned Probe

A pinned probe persists until closed explicitly.

Properties:

- remains visible while the operator works elsewhere
- can be repositioned and resized
- survives tile focus changes
- may be restored on app restart if session restoration exists

Pinned probes are for watch tasks, not for replacing the grid.

## Core Behaviors

### Open Probe

Open triggers:

- click probe button on tile
- keyboard shortcut on selected tile
- context menu action

Default behavior:

- open as a peek probe
- place near source tile
- give the probe focus
- visually link it to the source tile

If the same lens is already open as a probe for that session:

- focus the existing probe instead of opening duplicates by default

### Close Probe

Close triggers:

- close button
- `Esc` for peek probe
- command palette or keyboard close action

Closing a probe:

- removes the overlay
- returns focus to the source tile if the probe had focus
- removes source highlighting tied to the probe

### Pin Probe

Pinning converts a transient probe into a persistent one.

Pin behavior:

- preserve position and size
- keep source linkage visible
- remove auto-dismiss behaviors

### Focus Behavior

There are three distinct focus targets:

- selected tile
- active terminal within the selected tile
- active probe

Rules:

- a tile can be selected without terminal input focus
- probe focus never hides which tile remains the source
- switching focus between tile and probe should not surprise the terminal app
- returning from probe to terminal should be a single action

### Tile Lens Switching

Tiles may support temporary lens switching when the operator wants the tile itself to show something other than the terminal.

Rules:

- TUI remains the default lens
- switching away from TUI is reversible in one step
- alternate tile lenses are temporary and should not be sticky across sessions by default
- lens switching should not be required for normal inspection if probes are available

Tile lens switching is secondary to probes, not a substitute for them.

## Interaction Model

### Mouse

Mouse expectations:

- click tile to select
- click terminal area to focus terminal input
- click probe button to open probe
- drag probe by title bar
- resize probe by edges or corners
- click close button to dismiss
- click pin button to persist

Double-click should not be required for core actions.

### Keyboard

Keyboard must support fast supervision without heavy mouse dependence.

Required actions:

- move selection across tiles
- focus selected tile terminal
- open probe for current lens
- cycle probe lens
- pin/unpin probe
- close focused probe
- jump from focused probe back to source tile
- cycle sessions needing attention

Suggested defaults:

- arrow keys or `hjkl` for tile navigation
- `Enter` to focus terminal
- `p` to open default probe
- `1` `2` `3` to open specific lenses
- `Tab` to cycle focus between tile and probe
- `Esc` to close peek probe or leave terminal focus

Exact bindings can change, but the model should preserve one-step transitions between scan, inspect, and intervene.

## Main User Flows

### 1. Scan the Grid

Goal: identify which session deserves attention first.

Flow:

1. Operator sees the full grid of live sessions.
2. Each tile shows native terminal content plus compact status signals.
3. Sessions with `Attention`, `Blocked`, or `Failed` stand out visually.
4. Operator selects the most urgent session.

Success criteria:

- no probe is needed for first-pass prioritization
- tile chrome is enough to rank urgency

### 2. Inspect a Session Quickly

Goal: understand what is happening without leaving the grid.

Flow:

1. Operator selects a tile.
2. Operator opens a peek probe.
3. Probe appears near the source tile with clear visual linkage.
4. Operator inspects stdout, events, or process tree.
5. Operator closes the probe and returns to the grid.

Success criteria:

- inspection takes one or two actions
- the source tile remains visually obvious
- returning to the grid is instant

### 3. Watch Output While Supervising Others

Goal: keep monitoring one session while continuing broader supervision.

Flow:

1. Operator opens a stdout probe on a session of interest.
2. Operator pins the probe.
3. Probe remains visible while the operator navigates other tiles.
4. Operator closes the probe when the watch task ends.

Success criteria:

- the pinned probe does not force a layout switch
- the operator can still navigate the grid efficiently

### 4. Investigate a Suspicious Subprocess

Goal: determine whether the session launched an unexpected or stuck child process.

Flow:

1. Operator selects a tile showing `Blocked` or `Attention`.
2. Operator opens the process tree probe.
3. Operator inspects recent child process changes and current tree shape.
4. Operator decides whether to intervene in the terminal.

Success criteria:

- the process tree is readable enough to reveal structure quickly
- no deep integration with the agent is required

### 5. Intervene in the Native TUI

Goal: provide input or corrective action directly in the real agent terminal.

Flow:

1. Operator identifies a session requiring input.
2. Operator closes or background-leaves any probe.
3. Operator focuses the session terminal.
4. Operator interacts directly with the embedded native TUI.

Success criteria:

- intervention always happens in the real terminal surface
- no custom abstraction stands between operator and agent

## Visual Guidance

### Status Signaling

Status should use a compact combination of:

- color
- label
- subtle motion only when necessary

Use motion sparingly. Persistent animation across many tiles will become noise.

### Density

The grid should tolerate many sessions without collapsing into unreadable dashboards.

Default density guidance:

- prioritize terminal body area over chrome
- keep labels short
- prefer one-line summaries
- make probes carry the deeper detail load

### Empty and Small States

When tile size is constrained:

- preserve the terminal surface first
- collapse secondary labels
- keep the status signal and session identity visible

When no probes are open:

- the grid should look clean and calm

## Error and Edge Cases

### Probe Placement Conflicts

If opening a probe near a tile would render it mostly off-screen:

- shift it inward automatically
- preserve visible source linkage

### Many Pinned Probes

If multiple probes are pinned:

- allow overlap
- maintain z-order on focus
- provide a simple "close all probes" action

Do not auto-reflow the whole grid around them in v1.

### Session Disconnect

If a session backend disconnects:

- tile enters `Detached`
- terminal surface freezes with clear state indication
- probes remain open but show stale-data status

### Ambiguous Observability

If a lens depends on uncertain attribution:

- label the uncertainty directly in the UI
- prefer "session output" over falsely precise claims

## Engineering Notes

Recommended implementation direction:

- Rust application
- GTK4/libadwaita frontend
- VTE per session tile for real embedded terminals
- overlay widgets for probes
- backend for PTY lifecycle, process tracking, and event derivation

High-level architectural split:

- terminal/session host
- session model and state derivation
- probe manager
- tile/grid layout manager
- observability adapters

## V1 Scope

Must-have:

- grid of live embedded terminal sessions
- compact per-tile status
- selectable tiles
- stdout probe
- process tree probe
- events probe
- peek and pinned probe modes
- clear probe-to-tile linkage
- keyboard and mouse support for scan, inspect, and intervene

Should-have:

- heuristic `Blocked` and `Attention` signals
- session event timeline
- basic persistence of pinned probes and session layout

Out of scope for v1:

- deep agent-specific protocol integrations
- IDE-like code navigation
- orchestration workflows beyond session supervision
- perfect per-subprocess output attribution
- dense per-tile dashboards

## Acceptance Criteria

The UX is successful if:

- operators can supervise multiple terminal-native agents without losing the native TUI
- the grid remains the main working posture
- deeper inspection happens through probes, not through permanent tile clutter
- the product delivers useful observability without requiring deep hooks into every agent
- an operator can scan, inspect, watch, and intervene in one continuous flow
