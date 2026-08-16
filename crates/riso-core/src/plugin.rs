//! Extending riso without changing it.
//!
//! A plugin teaches riso to theme an application it does not know about. It is
//! a directory with a manifest and its templates, installed from git like a
//! theme, and it writes to the application's own config path rather than into
//! the generated theme.
//!
//! That last point is the difference from a built-in template: built-ins
//! produce files inside the theme, which the desktop then reads. A plugin
//! reaches an application that reads nothing but its own config.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::atomic::write_atomic;
use crate::error::IoError;
use crate::palette::Palette;
use crate::reload::{Executor, ReloadError};
use crate::template;

const MANIFEST_FILE: &str = "manifest.toml";

/// The manifest version this build understands.
pub const API_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("{0}: manifest is not valid TOML: {1}")]
    Manifest(PathBuf, #[source] toml::de::Error),
    #[error("{0}: manifest declares api {1}, this riso understands {2}")]
    Api(PathBuf, u32, u32),
    #[error("{0}: id '{1}' is not usable as a directory name")]
    Id(PathBuf, String),
    #[error("{0}: target '{1}' must be an absolute path or start with ~/")]
    Target(PathBuf, String),
    #[error("{0}: template '{1}' must be a relative path inside the plugin")]
    Template(PathBuf, String),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Reload(#[from] ReloadError),
}

/// One file a plugin generates.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RenderTarget {
    /// Template file, relative to the plugin directory.
    pub template: String,
    /// Where the rendered file goes. `~/` is expanded.
    pub target: String,
}

/// A plugin manifest.
///
/// TOML puts every key after a `[[render]]` header inside that table, so the
/// top-level fields have to come first.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub api: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub render: Vec<RenderTarget>,
    /// Command run after the files are written, as program and arguments.
    #[serde(default)]
    pub reload: Vec<String>,
    /// Skip the plugin entirely when this program is not on PATH.
    #[serde(default)]
    pub requires: Option<String>,
}

impl Manifest {
    pub fn parse(source: &str, at: &Path) -> Result<Self, PluginError> {
        let manifest: Self =
            toml::from_str(source).map_err(|e| PluginError::Manifest(at.to_path_buf(), e))?;

        if manifest.api != API_VERSION {
            return Err(PluginError::Api(
                at.to_path_buf(),
                manifest.api,
                API_VERSION,
            ));
        }
        if !crate::catalog::is_safe_name(&manifest.id) {
            return Err(PluginError::Id(at.to_path_buf(), manifest.id));
        }
        for render in &manifest.render {
            if !is_safe_target(&render.target) {
                return Err(PluginError::Target(at.to_path_buf(), render.target.clone()));
            }
            if !is_safe_template(&render.template) {
                return Err(PluginError::Template(
                    at.to_path_buf(),
                    render.template.clone(),
                ));
            }
        }
        Ok(manifest)
    }
}

/// A plugin as found on disk.
#[derive(Debug, Clone)]
pub struct Plugin {
    pub manifest: Manifest,
    pub dir: PathBuf,
}

/// What applying a plugin produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    pub id: String,
    pub written: Vec<PathBuf>,
    pub skipped: Option<String>,
}

/// Load every plugin in the given directories, weakest first.
///
/// A directory without a manifest is not a plugin and is passed over; a
/// manifest that will not parse is an error, because silently ignoring it
/// would look exactly like the plugin doing nothing.
pub fn discover(plugin_dirs: &[PathBuf]) -> Result<Vec<Plugin>, PluginError> {
    let mut found: Vec<Plugin> = Vec::new();

    for dir in plugin_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();

        for path in paths {
            let manifest_path = path.join(MANIFEST_FILE);
            if !manifest_path.is_file() {
                continue;
            }
            let source = std::fs::read_to_string(&manifest_path)
                .map_err(|e| IoError::Read(manifest_path.clone(), e))?;
            let manifest = Manifest::parse(&source, &manifest_path)?;

            // A later directory overrides an earlier plugin with the same id,
            // which is how a user edits one without touching what shipped it.
            if let Some(existing) = found.iter_mut().find(|p| p.manifest.id == manifest.id) {
                *existing = Plugin {
                    manifest,
                    dir: path,
                };
            } else {
                found.push(Plugin {
                    manifest,
                    dir: path,
                });
            }
        }
    }

    Ok(found)
}

/// Render a plugin's files and reload whatever reads them.
///
/// `store` records what was at each target beforehand, so uninstalling can
/// put it back. A plugin writes into config that belongs to someone else, so
/// this is where losing it would be easiest.
pub fn apply(
    plugin: &Plugin,
    palette: &Palette,
    home: &Path,
    store: Option<&mut crate::snapshot::Store>,
    exec: &dyn Executor,
) -> Result<Applied, PluginError> {
    let mut applied = Applied {
        id: plugin.manifest.id.clone(),
        ..Default::default()
    };

    if let Some(required) = &plugin.manifest.requires {
        if !on_path(required, home) {
            applied.skipped = Some(format!("{required} is not installed"));
            return Ok(applied);
        }
    }

    let mut store = store;
    for render in &plugin.manifest.render {
        let source_path = plugin.dir.join(&render.template);
        let source = std::fs::read_to_string(&source_path)
            .map_err(|e| IoError::Read(source_path.clone(), e))?;

        let target = expand_home(&render.target, home);
        if let Some(store) = store.as_deref_mut() {
            store.capture(&target)?;
        }

        // A plugin file lands among configuration the user owns, so it says
        // what it is: edits there are lost on the next theme change and
        // belong in the user's own config, which should include this file.
        let mut contents = banner(&target, &plugin.manifest.id).unwrap_or_default();
        contents.push_str(&template::render(&source, palette));
        write_atomic(&target, &contents)?;
        applied.written.push(target);
    }

    if let Some((program, args)) = plugin.manifest.reload.split_first() {
        exec.run(program, args)?;
    }

    Ok(applied)
}

/// The warning written at the top of a plugin's output, in whatever comment
/// syntax the target's format speaks. A format without comments, JSON above
/// all, gets none rather than a broken file.
fn banner(target: &Path, plugin_id: &str) -> Option<String> {
    let first = format!("Generated by riso plugin '{plugin_id}'; rewritten on every theme change.");
    let second = "Edits here are lost: keep them in your own config and include this file.";

    let extension = target.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "toml" | "ini" | "conf" | "cfg" | "yaml" | "yml" | "theme" => {
            Some(format!("# {first}\n# {second}\n"))
        }
        "rasi" => Some(format!("// {first}\n// {second}\n")),
        "lua" => Some(format!("-- {first}\n-- {second}\n")),
        "css" | "scss" => Some(format!("/* {first}\n   {second} */\n")),
        "vim" => Some(format!("\" {first}\n\" {second}\n")),
        _ => None,
    }
}

/// `~/x` means `x` under the home directory; anything else is taken as given.
pub fn expand_home(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(path),
    }
}

/// No path component may be `..`: checked per component, so a file name that
/// merely contains two dots stays legal.
fn climbs(path: &str) -> bool {
    path.split('/').any(|component| component == "..")
}

/// A target must be somewhere nameable, not a relative path whose meaning
/// depends on where riso happened to be run from.
fn is_safe_target(target: &str) -> bool {
    (target.starts_with('/') || target.starts_with("~/")) && !climbs(target)
}

/// A template is read from the plugin's own directory and may not reach out
/// of it: the target is validated, so its source has to be too.
fn is_safe_template(template: &str) -> bool {
    !template.is_empty()
        && !template.starts_with('/')
        && !template.starts_with("~/")
        && !climbs(template)
}

fn on_path(program: &str, _home: &Path) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reload::RecordingExecutor;

    fn palette() -> Palette {
        let (palette, _) = Palette::parse("background = \"#1a1b26\"\n");
        let (palette, _) = palette.resolve(false);
        palette
    }

    fn plugin_at(dir: &Path, id: &str, body: &str) -> PathBuf {
        let path = dir.join(id);
        std::fs::create_dir_all(&path).expect("mkdir");
        std::fs::write(path.join(MANIFEST_FILE), body).expect("write");
        path
    }

    #[test]
    fn parses_a_manifest() {
        // Top-level keys come before the first [[render]]: anything after it
        // belongs to that table, which is the mistake a manifest author makes
        // once.
        let manifest = Manifest::parse(
            r#"
            id = "eldios.zed"
            api = 1
            name = "Zed"
            reload = ["zed", "--reload"]

            [[render]]
            template = "zed.json.tpl"
            target = "~/.config/zed/themes/riso.json"
            "#,
            Path::new("/t/manifest.toml"),
        )
        .expect("manifest");

        assert_eq!(manifest.id, "eldios.zed");
        assert_eq!(manifest.render.len(), 1);
        assert_eq!(manifest.reload, ["zed", "--reload"]);
    }

    #[test]
    fn refuses_a_manifest_from_a_future_api() {
        let error = Manifest::parse("id = \"x\"\napi = 99\n", Path::new("/t/m.toml"));
        assert!(matches!(error, Err(PluginError::Api(_, 99, 1))));
    }

    #[test]
    fn refuses_an_id_that_would_escape_its_directory() {
        let error = Manifest::parse("id = \"../evil\"\napi = 1\n", Path::new("/t/m.toml"));
        assert!(matches!(error, Err(PluginError::Id(_, _))));
    }

    #[test]
    fn refuses_a_target_that_is_relative_or_climbs_out() {
        for target in ["relative/path.conf", "~/../../etc/passwd", "~/ok/../../x"] {
            let error = Manifest::parse(
                &format!(
                    "id = \"x\"\napi = 1\n[[render]]\ntemplate = \"t\"\ntarget = \"{target}\"\n"
                ),
                Path::new("/t/m.toml"),
            );
            assert!(
                matches!(error, Err(PluginError::Target(_, _))),
                "{target} should be refused"
            );
        }
    }

    #[test]
    fn refuses_a_template_that_reaches_out_of_the_plugin() {
        for template in [
            "../secrets.tpl",
            "/etc/passwd",
            "~/x.tpl",
            "a/../../x.tpl",
            "",
        ] {
            let error = Manifest::parse(
                &format!(
                    "id = \"x\"\napi = 1\n[[render]]\ntemplate = \"{template}\"\ntarget = \"~/.config/x\"\n"
                ),
                Path::new("/t/m.toml"),
            );
            assert!(
                matches!(error, Err(PluginError::Template(_, _))),
                "{template} should be refused"
            );
        }
        // Two dots inside a name are not a climb.
        assert!(is_safe_template("sub/foo..bar.tpl"));
        assert!(is_safe_target("~/.config/foo..bar.conf"));
    }

    #[test]
    fn discovers_plugins_and_lets_a_later_directory_win() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shipped = dir.path().join("shipped");
        let user = dir.path().join("user");
        plugin_at(&shipped, "a", "id = \"a\"\napi = 1\nname = \"shipped\"\n");
        plugin_at(&shipped, "b", "id = \"b\"\napi = 1\n");
        plugin_at(&user, "a", "id = \"a\"\napi = 1\nname = \"mine\"\n");
        std::fs::create_dir_all(shipped.join("not-a-plugin")).expect("mkdir");

        let found = discover(&[shipped, user]).expect("discover");

        assert_eq!(found.len(), 2);
        let a = found.iter().find(|p| p.manifest.id == "a").expect("a");
        assert_eq!(a.manifest.name.as_deref(), Some("mine"));
    }

    #[test]
    fn a_broken_manifest_is_reported_not_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        plugin_at(dir.path(), "bad", "this is not toml = = =");
        assert!(discover(&[dir.path().to_path_buf()]).is_err());
    }

    #[test]
    fn renders_into_the_declared_target_and_reloads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let plugins = dir.path().join("plugins");
        let path = plugin_at(
            &plugins,
            "zed",
            "id = \"zed\"\napi = 1\nreload = [\"touch\", \"/tmp/x\"]\n\
             [[render]]\ntemplate = \"t.tpl\"\ntarget = \"~/.config/zed/riso.json\"\n",
        );
        std::fs::write(path.join("t.tpl"), "{ \"bg\": \"{{ background }}\" }").expect("write");

        let plugins = discover(&[plugins]).expect("discover");
        let recorder = RecordingExecutor::default();
        let applied = apply(&plugins[0], &palette(), &home, None, &recorder).expect("apply");

        assert_eq!(
            std::fs::read_to_string(home.join(".config/zed/riso.json")).expect("read"),
            "{ \"bg\": \"#1a1b26\" }"
        );
        assert_eq!(applied.written.len(), 1);
        assert_eq!(recorder.calls(), ["touch /tmp/x"]);
    }

    #[test]
    fn skips_a_plugin_whose_application_is_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins = dir.path().join("plugins");
        plugin_at(
            &plugins,
            "absent",
            "id = \"absent\"\napi = 1\nrequires = \"definitely-not-installed-xyz\"\n",
        );

        let found = discover(&[plugins]).expect("discover");
        let recorder = RecordingExecutor::default();
        let applied = apply(&found[0], &palette(), dir.path(), None, &recorder).expect("apply");

        assert!(applied.skipped.is_some());
        assert!(applied.written.is_empty());
        assert!(recorder.calls().is_empty());
    }

    #[test]
    fn a_plugin_file_says_what_it_is_when_its_format_can() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let plugins = dir.path().join("plugins");
        let path = plugin_at(
            &plugins,
            "kv",
            "id = \"kv\"\napi = 1\n\
             [[render]]\ntemplate = \"t.tpl\"\ntarget = \"~/.config/app/riso.toml\"\n",
        );
        std::fs::write(path.join("t.tpl"), "bg = \"{{ background }}\"\n").expect("write");

        let plugins = discover(&[plugins]).expect("discover");
        apply(
            &plugins[0],
            &palette(),
            &home,
            None,
            &RecordingExecutor::default(),
        )
        .expect("apply");

        let written = std::fs::read_to_string(home.join(".config/app/riso.toml")).expect("read");
        assert!(written.starts_with("# Generated by riso plugin 'kv'"));
        assert!(written.contains("include this file"));
        assert!(written.ends_with("bg = \"#1a1b26\"\n"));
    }

    #[test]
    fn a_format_without_comments_gets_no_banner() {
        // JSON is the case that matters: a comment breaks the whole file.
        assert_eq!(banner(Path::new("/t/riso.json"), "x"), None);
        assert_eq!(banner(Path::new("/t/no-extension"), "x"), None);
        assert!(banner(Path::new("/t/riso.css"), "x")
            .expect("css comments exist")
            .starts_with("/*"));
    }

    #[test]
    fn expands_only_a_leading_tilde() {
        let home = Path::new("/home/x");
        assert_eq!(expand_home("~/a/b", home), PathBuf::from("/home/x/a/b"));
        assert_eq!(expand_home("/etc/a", home), PathBuf::from("/etc/a"));
    }
}
