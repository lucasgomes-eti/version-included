use std::fs;
use std::io;
use std::path::Path;

/// Read access to environment variables.
pub trait Env {
    /// The variable's value, or `None` if it is unset or not valid Unicode.
    fn var(&self, key: &str) -> Option<String>;
}

/// The filesystem operations `versinc` needs.
pub trait Fs {
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    /// Writes `contents` to `path`. Without `overwrite`, fails with
    /// [`io::ErrorKind::AlreadyExists`] if the file exists.
    fn write(&self, path: &Path, contents: &str, overwrite: bool) -> io::Result<()>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
}

/// The process environment.
pub struct SystemEnv;

impl Env for SystemEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// The real filesystem.
pub struct SystemFs;

impl Fs for SystemFs {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn write(&self, path: &Path, contents: &str, overwrite: bool) -> io::Result<()> {
        use io::Write;
        let mut file = if overwrite {
            fs::File::create(path)?
        } else {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?
        };
        file.write_all(contents.as_bytes())
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;

    pub(crate) struct FakeEnv(HashMap<String, String>);

    impl FakeEnv {
        pub(crate) fn new(vars: &[(&str, &str)]) -> Self {
            FakeEnv(
                vars.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            )
        }
    }

    impl Env for FakeEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
    }

    /// In-memory filesystem. Directories are implicit; writes beneath
    /// `read_only_dirs` and reads of `unreadable` files fail.
    #[derive(Default)]
    pub(crate) struct FakeFs {
        pub(crate) files: RefCell<HashMap<PathBuf, String>>,
        pub(crate) dirs: RefCell<Vec<PathBuf>>,
        pub(crate) read_only_dirs: Vec<PathBuf>,
        pub(crate) unreadable: Vec<PathBuf>,
    }

    impl FakeFs {
        pub(crate) fn with_files(files: &[(&str, &str)]) -> Self {
            let fs = FakeFs::default();
            for (path, text) in files {
                fs.files
                    .borrow_mut()
                    .insert(PathBuf::from(path), text.to_string());
            }
            fs
        }

        pub(crate) fn file(&self, path: &str) -> Option<String> {
            self.files.borrow().get(Path::new(path)).cloned()
        }

        fn check_writable(&self, path: &Path) -> io::Result<()> {
            if self.read_only_dirs.iter().any(|dir| path.starts_with(dir)) {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "read-only"));
            }
            Ok(())
        }
    }

    impl Fs for FakeFs {
        fn read_to_string(&self, path: &Path) -> io::Result<String> {
            if self.unreadable.iter().any(|p| p == path) {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
            }
            self.files
                .borrow()
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not found"))
        }

        fn write(&self, path: &Path, contents: &str, overwrite: bool) -> io::Result<()> {
            self.check_writable(path)?;
            let mut files = self.files.borrow_mut();
            if !overwrite && files.contains_key(path) {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, "exists"));
            }
            files.insert(path.to_path_buf(), contents.to_string());
            Ok(())
        }

        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            self.check_writable(path)?;
            self.dirs.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }

    #[test]
    fn system_env_reads_process_environment() {
        // PATH is set in every environment the tests run in.
        assert!(SystemEnv.var("PATH").is_some());
        assert_eq!(SystemEnv.var("VERSINC_TEST_SURELY_UNSET_VARIABLE"), None);
    }

    #[test]
    fn system_fs_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b");
        SystemFs.create_dir_all(&nested).unwrap();
        let file = nested.join("f.txt");

        SystemFs.write(&file, "one", false).unwrap();
        assert_eq!(SystemFs.read_to_string(&file).unwrap(), "one");

        let err = SystemFs.write(&file, "two", false).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(SystemFs.read_to_string(&file).unwrap(), "one");

        SystemFs.write(&file, "three", true).unwrap();
        assert_eq!(SystemFs.read_to_string(&file).unwrap(), "three");
    }

    #[test]
    fn system_fs_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let err = SystemFs
            .read_to_string(&dir.path().join("missing"))
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
