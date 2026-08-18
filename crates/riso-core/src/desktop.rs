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
    /// Quickshell configuration directory of the running shell, when there is
    /// one. Given it, riso talks to Quickshell directly instead of going
    /// through a desktop's own wrapper script.
    pub shell_config: Option<std::path::PathBuf>,
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
            //
            // Omarchy's own `omarchy-shell` is a wrapper over this same
            // Quickshell call, so knowing the config directory lets riso skip
            // it and talk to Quickshell, which is nobody's in particular.
            Self::Omarchy => {
                let colors = payload.colors.as_deref().map(base64).unwrap_or_default();
                let shell = payload.shell.as_deref().map(base64).unwrap_or_default();

                match &payload.shell_config {
                    Some(dir) => exec.run(
                        "qs",
                        &[
                            "ipc".to_owned(),
                            "-n".to_owned(),
                            "-p".to_owned(),
                            dir.to_string_lossy().into_owned(),
                            "call".to_owned(),
                            "--".to_owned(),
                            "shell".to_owned(),
                            "applyTheme".to_owned(),
                            colors,
                            shell,
                        ],
                    ),
                    None => exec.run(
                        "omarchy-shell",
                        &["shell".to_owned(), "applyTheme".to_owned(), colors, shell],
                    ),
                }
            }
            Self::Hyprland => exec.run("hyprctl", &owned(&["reload"])),
            Self::Sway => exec.run("swaymsg", &owned(&["reload"])),
            Self::Niri => exec.run("niri", &owned(&["msg", "action", "do-screen-transition"])),
            Self::None => Ok(()),
        }
    }

    /// Run the desktop's own after-theme hooks, when it has any.
    ///
    /// Omarchy retints applications through named helper commands, and its
    /// own theme pipeline runs this same list after a switch. Each is best
    /// effort: a helper this installation does not carry is not an error,
    /// and one that fails does not stop the rest.
    pub fn post_apply(self, exec: &dyn Executor) {
        if self != Self::Omarchy {
            return;
        }
        const OMARCHY_POST_APPLY: &[&str] = &[
            "omarchy-restart-terminal",
            "omarchy-restart-hyprctl",
            "omarchy-restart-btop",
            "omarchy-restart-opencode",
            "omarchy-restart-helix",
            "omarchy-theme-set-foot",
            "omarchy-theme-set-tmux",
            "omarchy-theme-set-gnome",
            "omarchy-theme-set-pi",
            "omarchy-theme-set-claude",
            "omarchy-theme-set-browser",
            "omarchy-theme-set-vscode",
            "omarchy-theme-set-obsidian",
            "omarchy-theme-set-keyboard",
        ];
        for hook in OMARCHY_POST_APPLY {
            let _ = exec.run(hook, &[]);
        }
    }

    /// Hand the desktop a new wallpaper.
    ///
    /// Only Omarchy draws its own: a bare compositor leaves wallpapers to
    /// whatever daemon the user runs, which is not riso's to guess.
    pub fn set_background(
        self,
        exec: &dyn Executor,
        image: &std::path::Path,
        shell_config: Option<&std::path::Path>,
    ) -> Result<(), ReloadError> {
        if self != Self::Omarchy {
            return Ok(());
        }
        let image = image.to_string_lossy().into_owned();
        match shell_config {
            Some(dir) => exec.run(
                "qs",
                &[
                    "ipc".to_owned(),
                    "-n".to_owned(),
                    "-p".to_owned(),
                    dir.to_string_lossy().into_owned(),
                    "call".to_owned(),
                    "--".to_owned(),
                    "background".to_owned(),
                    "set".to_owned(),
                    image,
                ],
            ),
            None => exec.run(
                "omarchy-shell",
                &[
                    "-q".to_owned(),
                    "background".to_owned(),
                    "set".to_owned(),
                    image,
                ],
            ),
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
    fn talks_to_quickshell_directly_when_it_knows_where_it_lives() {
        let recorder = RecordingExecutor::default();
        let payload = Payload {
            colors: Some(b"foo".to_vec()),
            shell: Some(b"bar".to_vec()),
            shell_config: Some(std::path::PathBuf::from("/usr/share/omarchy/shell")),
        };

        Desktop::Omarchy
            .reload(&recorder, &payload)
            .expect("reload");

        assert_eq!(
            recorder.calls(),
            ["qs ipc -n -p /usr/share/omarchy/shell call -- shell applyTheme Zm9v YmFy"]
        );
    }

    #[test]
    fn hands_omarchy_the_palette_encoded() {
        let recorder = RecordingExecutor::default();
        let payload = Payload {
            colors: Some(b"foo".to_vec()),
            shell: Some(b"bar".to_vec()),
            shell_config: None,
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
    fn hands_omarchy_the_wallpaper_and_nobody_else() {
        let recorder = RecordingExecutor::default();
        Desktop::Omarchy
            .set_background(&recorder, std::path::Path::new("/b/img.png"), None)
            .expect("set");
        assert_eq!(
            recorder.calls(),
            ["omarchy-shell -q background set /b/img.png"]
        );

        let recorder = RecordingExecutor::default();
        Desktop::Hyprland
            .set_background(&recorder, std::path::Path::new("/b/img.png"), None)
            .expect("set");
        assert!(recorder.calls().is_empty());
    }

    #[test]
    fn omarchy_runs_its_retint_hooks_and_nobody_else_does() {
        let recorder = RecordingExecutor::default();
        Desktop::Omarchy.post_apply(&recorder);
        let calls = recorder.calls();
        let ran = |name: &str| calls.iter().any(|c| c.trim_end() == name);
        assert!(ran("omarchy-restart-terminal"));
        assert!(ran("omarchy-theme-set-vscode"));

        let recorder = RecordingExecutor::default();
        Desktop::Hyprland.post_apply(&recorder);
        assert!(recorder.calls().is_empty());
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
