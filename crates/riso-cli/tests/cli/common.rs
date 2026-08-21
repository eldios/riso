//! A throwaway home for one test: every path riso resolves lands inside
//! it, and PATH holds only the binaries the test planted, so what the
//! host has installed never leaks into an assertion.

use std::path::PathBuf;
use std::process::{Command, Output};

pub struct Sandbox {
    root: tempfile::TempDir,
}

impl Sandbox {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(root.path().join("bin")).expect("mkdir");
        Sandbox { root }
    }

    pub fn home(&self) -> PathBuf {
        self.root.path().to_path_buf()
    }

    pub fn state(&self) -> PathBuf {
        self.home().join(".local/state/riso")
    }

    /// Plant a fake executable so PATH probes see the tool.
    pub fn fake_bin(&self, name: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.home().join("bin").join(name);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    /// A file under the sandbox home, parents included.
    pub fn write(&self, rel: &str, content: &str) -> PathBuf {
        let path = self.home().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, content).expect("write");
        path
    }

    /// A minimal installed theme in the sandbox's data dir.
    pub fn theme(&self, name: &str) -> PathBuf {
        let dir = self.home().join(".local/share/riso/themes").join(name);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(
            dir.join("colors.toml"),
            "background = \"#1a1b26\"\n\
             foreground = \"#a9b1d6\"\n\
             accent = \"#7aa2f7\"\n\
             mode = \"dark\"\n",
        )
        .expect("write");
        dir
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_riso"));
        cmd.env_clear()
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("XDG_DATA_HOME", self.home().join(".local/share"))
            .env("XDG_STATE_HOME", self.home().join(".local/state"))
            .env("PATH", self.home().join("bin"))
            .env("RISO_DECLARATIVE", "0");
        // The coverage profile sink must survive the scrub, or runs
        // spawned here never write their counters.
        if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
            cmd.env("LLVM_PROFILE_FILE", profile);
        }
        cmd
    }

    /// A fake executable that appends its arguments to calls-<name>.log.
    pub fn logging_bin(&self, name: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.home().join("bin").join(name);
        std::fs::write(
            &path,
            format!("#!/bin/sh\necho \"$@\" >> \"$HOME/calls-{name}.log\"\n"),
        )
        .expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    pub fn calls(&self, name: &str) -> String {
        std::fs::read_to_string(self.home().join(format!("calls-{name}.log"))).unwrap_or_default()
    }

    /// Link a host binary into the sandbox PATH; the test process's own
    /// PATH says where it lives.
    pub fn real_bin(&self, name: &str) {
        let host = std::env::var_os("PATH")
            .and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join(name))
                    .find(|p| p.is_file())
            })
            .unwrap_or_else(|| panic!("{name} not on the test host's PATH"));
        std::os::unix::fs::symlink(host, self.home().join("bin").join(name)).expect("symlink");
    }

    /// A committed git repository holding one theme, for install tests.
    pub fn git_theme_repo(&self, name: &str) -> PathBuf {
        let repo = self.home().join("repos").join(name);
        std::fs::create_dir_all(&repo).expect("mkdir");
        std::fs::write(
            repo.join("colors.toml"),
            "background = \"#1a1b26\"\nforeground = \"#a9b1d6\"\naccent = \"#7aa2f7\"\nmode = \"dark\"\n",
        )
        .expect("write");
        self.git(&repo, &["init", "-q", "-b", "main"]);
        self.git(&repo, &["add", "."]);
        self.git(&repo, &["commit", "-q", "-m", "theme"]);
        repo
    }

    pub fn git(&self, repo: &std::path::Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    }

    pub fn riso(&self, args: &[&str]) -> Output {
        self.command().args(args).output().expect("run riso")
    }

    /// Same run with one extra environment variable.
    pub fn riso_env(&self, args: &[&str], key: &str, value: &str) -> Output {
        self.command()
            .args(args)
            .env(key, value)
            .output()
            .expect("run riso")
    }
}

pub fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
