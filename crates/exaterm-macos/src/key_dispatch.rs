#[cfg(target_os = "macos")]
mod imp {
    /// Action the event monitor should take for a keyboard event.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum KeyAction {
        /// Pass the event through to the AppKit responder chain.
        PassThrough,
        /// Swallow the event so no responder sees it.
        Consume,
    }

    /// Returns the action for a key event directed at an embedded battlefield terminal.
    ///
    /// When the selected session is embedded (visible) in the battlefield view, all
    /// keys — including Return — must reach SwiftTerm as first responder so they are
    /// forwarded to the shell process.  The caller is responsible for calling
    /// `makeFirstResponder` before consulting this function for Return (key code 36).
    pub fn embedded_terminal_key_action(_key_code: u16) -> KeyAction {
        KeyAction::PassThrough
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn return_passes_through_to_embedded_terminal() {
            assert_eq!(embedded_terminal_key_action(36), KeyAction::PassThrough);
        }

        #[test]
        fn all_keys_pass_through_to_embedded_terminal() {
            for key_code in [0u16, 36, 48, 53, 65, 97, 125, 126] {
                assert_eq!(
                    embedded_terminal_key_action(key_code),
                    KeyAction::PassThrough,
                    "key code {key_code} should pass through to embedded terminal"
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use imp::*;
