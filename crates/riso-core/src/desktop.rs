//! Telling a desktop that the theme changed.
//!
//! Every desktop has its own way of being told, and some have none. Keeping
//! that knowledge in one enum is what stops the rest of riso from assuming a
//! particular desktop is running.

use crate::reload::{base64, Executor, ReloadError};

/// The desktops riso knows how to notify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Desktop {
    /// Omarchy 4: one Quickshell process, retints itself from a palette.
    Omarchy,
    /// Hyprland on its own: rereads its config.
    Hyprland,
    Sway,
    Niri,
    /// Write the files and stop. Correct for a desktop that watches them, and
    /// the only honest answer when nothing is recognized.
    #[default]
    None,
}

/// What a desktop may need to be handed along with the news.
#[derive(Debug, Clone, Default)]
pub struct Payload {
    /// Contents of the theme's `colors.toml`.
    pub colors: Option<Vec<u8>>,
    /// Contents of the theme's `shell.toml`.
    pub shell: Option<Vec<u8>>,
}

impl Desktop {
    /// The name used on the command line and in configuration.
    pub fn name(self) -> &'static str {
        match self {
            Self::Omarchy => "omarchy",
            Self::Hyprland => "hyprland",
            Self::Sway => "sway",
            Self::Niri => "niri",
            Self::None => "none",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "omarchy" => Some(Self::Omarchy),
            "hyprland" => Some(Self::Hyprland),
            "sway" => Some(Self::Sway),
            "niri" => Some(Self::Niri),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    /// Recognize the running desktop from the session's environment.
    pub fn detect() -> Self {
        Self::detect_with(|key| std::env::var(key).ok())
    }

    /// Order matters: Omarchy runs on Hyprland, so it has to be checked first
    /// or it would be seen as plain Hyprland and told the wrong thing.
    pub fn detect_with(var: impl Fn(&str) -> Option<String>) -> Self {
        let set = |key: &str| var(key).is_some_and(|value| !value.is_empty());

        if set("OMARCHY_PATH") {
            Self::Omarchy
        } else if set("NIRI_SOCKET") {
            Self::Niri
        } else if set("SWAYSOCK") {
            Self::Sway
        } else if set("HYPRLAND_INSTANCE_SIGNATURE") {
            Self::Hyprland
        } else {
            Self::None
        }
    }

    /// Tell the desktop to pick up the new theme.
    pub fn reload(self, exec: &dyn Executor, payload: &Payload) -> Result<(), ReloadError> {
        let owned = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();

        match self {
            // Handing over the palette is what lets the shell retint without
            // restarting; base64 keeps it to a single argument.
            Self::Omarchy => exec.run(
                "omarchy-shell",
                &[
                    "shell".to_owned(),
                    "applyTheme".to_owned(),
                    payload.colors.as_deref().map(base64).unwrap_or_default(),
                    payload.shell.as_deref().map(base64).unwrap_or_default(),
                ],
            ),
            Self::Hyprland => exec.run("hyprctl", &owned(&["reload"])),
            Self::Sway => exec.run("swaymsg", &owned(&["reload"])),
            Self::Niri => exec.run("niri", &owned(&["msg", "action", "do-screen-transition"])),
            Self::None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reload::RecordingExecutor;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn recognizes_each_desktop_from_its_own_marker() {
        assert_eq!(
            Desktop::detect_with(env(&[("SWAYSOCK", "/run/sway.sock")])),
            Desktop::Sway
        );
        assert_eq!(
            Desktop::detect_with(env(&[("NIRI_SOCKET", "/run/niri.sock")])),
            Desktop::Niri
        );
        assert_eq!(
            Desktop::detect_with(env(&[("HYPRLAND_INSTANCE_SIGNATURE", "abc")])),
            Desktop::Hyprland
        );
    }

    #[test]
    fn omarchy_wins_over_the_compositor_it_runs_on() {
        let both = env(&[
            ("HYPRLAND_INSTANCE_SIGNATURE", "abc"),
            ("OMARCHY_PATH", "/usr/share/omarchy"),
        ]);
        assert_eq!(Desktop::detect_with(both), Desktop::Omarchy);
    }

    #[test]
    fn an_unknown_session_notifies_nothing() {
        assert_eq!(Desktop::detect_with(env(&[])), Desktop::None);
        assert_eq!(
            Desktop::detect_with(env(&[("SWAYSOCK", "")])),
            Desktop::None,
            "an empty marker is not a running desktop"
        );
    }

    #[test]
    fn names_round_trip() {
        for desktop in [
            Desktop::Omarchy,
            Desktop::Hyprland,
            Desktop::Sway,
            Desktop::Niri,
            Desktop::None,
        ] {
            assert_eq!(Desktop::from_name(desktop.name()), Some(desktop));
        }
        assert_eq!(Desktop::from_name("OMARCHY"), Some(Desktop::Omarchy));
        assert_eq!(Desktop::from_name("gnome"), None);
    }

    #[test]
    fn hands_omarchy_the_palette_encoded() {
        let recorder = RecordingExecutor::default();
        let payload = Payload {
            colors: Some(b"foo".to_vec()),
            shell: Some(b"bar".to_vec()),
        };

        Desktop::Omarchy
            .reload(&recorder, &payload)
            .expect("reload");

        assert_eq!(
            recorder.calls(),
            ["omarchy-shell shell applyTheme Zm9v YmFy"]
        );
    }

    #[test]
    fn tells_each_compositor_in_its_own_words() {
        let cases = [
            (Desktop::Hyprland, "hyprctl reload"),
            (Desktop::Sway, "swaymsg reload"),
            (Desktop::Niri, "niri msg action do-screen-transition"),
        ];
        for (desktop, expected) in cases {
            let recorder = RecordingExecutor::default();
            desktop
                .reload(&recorder, &Payload::default())
                .expect("reload");
            assert_eq!(recorder.calls(), [expected]);
        }
    }

    #[test]
    fn none_runs_nothing_at_all() {
        let recorder = RecordingExecutor::default();
        Desktop::None
            .reload(&recorder, &Payload::default())
            .expect("reload");
        assert!(recorder.calls().is_empty());
    }
}
