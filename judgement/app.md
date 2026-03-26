# Exaterm

Exaterm is a Linux desktop app for supervising multiple terminal-native coding agents at once.

Its core job is to let an operator keep a grid of live agent sessions on screen, preserve each agent's native terminal UI as the primary surface, and open temporary probes when deeper inspection is needed.

The experience should be judged primarily as a supervisor for terminal-native workflows, not as a general terminal emulator and not as a replacement shell for Codex, Claude Code, or similar tools.

The most important parts of the experience are:

- Whether the grid supports fast scanning and prioritization across several live sessions.
- Whether each tile keeps the real terminal/TUI visually primary instead of burying it under management chrome.
- Whether session status is clear enough to reveal which agent is active, blocked, failed, or needs intervention first.
- Whether probes make deeper inspection feel fast and local to a tile rather than forcing a full layout change.
- Whether the operator can move from scan to inspect to direct intervention in the native terminal without friction or loss of orientation.

The workflows that deserve the most evaluation time are:

- Scanning a multi-session grid to identify which agent needs attention first.
- Opening and dismissing probes for stdout, process tree, or events without losing the surrounding grid context.
- Keeping a probe pinned while continuing to supervise other sessions.
- Returning from inspection to direct intervention in the embedded terminal for the selected session.

Supporting surfaces still matter, but less:

- session creation and launch controls
- menus, preferences, and workspace chrome
- theming and visual polish outside the core supervision loop
- any auxiliary dialogs that do not materially affect grid supervision

Quality looks like this:

- The app reads clearly as an agent supervisor, with sessions as the primary object and the grid as the main working posture.
- The live terminal in each tile remains dominant and legible.
- Important state changes are easy to spot without turning every tile into a miniature dashboard.
- Probes feel attached to a source tile, open quickly, are easy to dismiss, and provide useful depth on demand.
- Focus changes between tile, terminal, and probe are coherent and predictable.
- The operator can confidently tell what happened after opening a probe or intervening in a session.
- Dense multi-session layouts remain structurally legible after resizing the window to a realistic working size.

Weak quality looks like this:

- The product feels like a generic pane manager rather than a supervisor for agent runs.
- Tile chrome or observability widgets compete with the terminal and reduce the sense that the native TUI is primary.
- It is hard to tell which session matters most right now.
- Probes feel modal, disconnected from their source tile, or cumbersome enough that users avoid them.
- Focus, selection, and terminal input behavior are ambiguous or brittle.
- Deeper inspection requires abandoning the grid or switching into a different conceptual mode.
- Dense layouts become visually noisy or hard to parse once several sessions are visible at once.

When judging this product, spend more time on the core supervision loop than on supporting chrome. The strongest evidence will come from whether the app preserves orientation across scanning, probing, and intervening in a real terminal session.

Environment notes:

- This app targets Linux desktop usage.
- Prefer X11 for automation.
- Resize the main window to a realistic multi-tile working size before judging density, state clarity, or probe behavior.
