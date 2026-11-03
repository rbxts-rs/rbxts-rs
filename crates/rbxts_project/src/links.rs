//! Following links as Node's JavaScript `fs.realpathSync` does, which roblox-ts's Rojo resolver uses: one link at a
//! time, each relative target resolved against the link's real folder.
//!
//! Windows resolves a relative symbolic link against the path it was reached by instead. A relative link inside a
//! folder reached through a junction (bun's links in the `node_modules` of a workspace package that a project links
//! with a junction) then points nowhere for the system's `stat`, while Node's realpath finds it. On other systems the
//! kernel already resolves a relative link against its real folder, and the fallback below finds nothing more.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `fs::metadata` (links followed), with a relative link reached through a junction resolved against its real folder.
pub fn metadata(path: &Path) -> io::Result<fs::Metadata> {
    match fs::metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            real_path(path).and_then(fs::metadata).map_err(|_| error)
        }
        result => result,
    }
}

/// `fs::read_dir`, a folder the system cannot reach by this path listed by its real path, as roblox-ts lists
/// `fs.realpathSync(directory)`.
pub fn read_dir(path: &Path) -> io::Result<fs::ReadDir> {
    match fs::read_dir(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            real_path(path).and_then(fs::read_dir).map_err(|_| error)
        }
        result => result,
    }
}

/// `fs::canonicalize`, with a relative link reached through a junction resolved against its real folder: when the
/// system cannot resolve the path, its folder is resolved first (the same way, up to the first folder it can) and the
/// name is resolved from there.
pub fn real_path(path: &Path) -> io::Result<PathBuf> {
    match fs::canonicalize(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else { return Err(error) };
            if parent.as_os_str().is_empty() {
                return Err(error);
            }
            let Ok(real_parent) = real_path(parent) else { return Err(error) };
            fs::canonicalize(real_parent.join(name)).map_err(|_| error)
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// real/pkg/node_modules/link -> ..\..\store\target (relative), proj/j -> real/pkg (junction on Windows, a
    /// symbolic link elsewhere). Skipped where links cannot be made (Windows without developer mode or admin).
    #[test]
    fn a_relative_link_reached_through_a_junction_resolves_from_its_real_folder() {
        let root = std::env::temp_dir().join(format!("rbxts-links-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("real/pkg/node_modules")).unwrap();
        fs::create_dir_all(root.join("real/store/target")).unwrap();
        fs::write(root.join("real/store/target/x.txt"), "x").unwrap();
        fs::create_dir_all(root.join("proj")).unwrap();
        let relative = Path::new("..").join("..").join("store").join("target");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(&relative, root.join("real/pkg/node_modules/link")).is_ok()
            // mklink takes backslashes only
            && std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(root.join("proj").join("j"))
                .arg(root.join("real").join("pkg"))
                .output()
                .is_ok_and(|o| o.status.success());
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&relative, root.join("real/pkg/node_modules/link")).is_ok()
            && std::os::unix::fs::symlink(root.join("real/pkg"), root.join("proj/j")).is_ok();
        if !made {
            eprintln!("skipped: links cannot be made here");
            let _ = fs::remove_dir_all(&root);
            return;
        }
        let through = root.join("proj/j/node_modules/link");
        assert!(metadata(&through).unwrap().is_dir());
        assert_eq!(real_path(&through).unwrap(), fs::canonicalize(root.join("real/store/target")).unwrap());
        assert!(metadata(&through.join("x.txt")).unwrap().is_file());
        let names: Vec<_> = read_dir(&through).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, [std::ffi::OsString::from("x.txt")]);
        // a link that is broken from its real folder too stays an error
        let broken = root.join("real/pkg/node_modules/broken");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(Path::new("..").join("nowhere"), &broken).is_ok();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(Path::new("..").join("nowhere"), &broken).is_ok();
        assert!(made);
        let error = metadata(&root.join("proj/j/node_modules/broken")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(real_path(&root.join("proj/j/node_modules/broken")).is_err());
        // unlink the junction itself before removing the tree, so the removal never goes through it
        #[cfg(windows)]
        fs::remove_dir(root.join("proj/j")).unwrap();
        #[cfg(unix)]
        fs::remove_file(root.join("proj/j")).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
}
