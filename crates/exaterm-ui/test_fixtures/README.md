# Test Fixtures

Binary PTY captures used by the scrollback-band TUI tests.

## Capture procedure

Record a session using `script` (macOS/Linux):

```sh
script -q /dev/null -c claude claude_session.bin
# Interact for ~5 seconds, then exit.

script -q /dev/null -c codex codex_session.bin
# Interact for ~5 seconds, then exit.
```

The files committed here are synthetic substitutes that reproduce the
alt-screen structure (SMCUP + CUP-driven frame painting) at minimal size.
Replace with real captures whenever the TUI format changes substantially.

## Expected substrings

`claude_session.expected.txt` and `codex_session.expected.txt` list substrings
that must appear in the scrollback band after ingesting the corresponding binary.
Update these files whenever the fixture binary is recaptured.
