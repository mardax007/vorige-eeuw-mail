//! Rescue mode for a PST that `outlook-pst` cannot read (phase 3 task 5): the separately shipped
//! `readpst` (libpst, GPL-2.0-or-later, so it may ship beside our GPL-3.0 app) turns our copy of
//! the file into a folder of `.eml` files, which the usual folder reader then imports. `readpst`
//! only reads its input. What was read and what not may be lost on the way.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the installer puts the rescue tool: `rescue\readpst.exe` next to the app.
pub fn tool() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let tool = exe.parent()?.join("rescue").join(if cfg!(windows) { "readpst.exe" } else { "readpst" });
    tool.is_file().then_some(tool)
}

/// The arguments: folders as on disk (`-r`), one `.eml` per message (`-e`), only mail (`-t e`),
/// no RTF copies of the text as attachments (`-b`), quiet (`-q`).
fn args(pst: &Path, out: &Path) -> Vec<std::ffi::OsString> {
    let mut a: Vec<std::ffi::OsString> = ["-r", "-e", "-q", "-b", "-t", "e", "-o"].iter().map(Into::into).collect();
    a.push(out.into());
    a.push(pst.into());
    a
}

/// Run the tool on `pst` (our own copy) into the empty folder `out`.
pub fn convert(tool: &Path, pst: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("rescue folder: {}", e.kind()))?;
    let mut cmd = Command::new(tool);
    cmd.args(args(pst, out));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output().map_err(|e| format!("rescue tool did not start: {}", e.kind()))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        let said = said.lines().last().unwrap_or("").trim();
        return Err(format!("rescue tool stopped ({}): {said}", output.status));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readpst_is_asked_for_mail_only_as_eml_files_in_folders() {
        let a = args(Path::new("C:/kopie/Outlook.pst"), Path::new("C:/data/rescue"));
        let a: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert_eq!(a, ["-r", "-e", "-q", "-b", "-t", "e", "-o", "C:/data/rescue", "C:/kopie/Outlook.pst"]);
    }

    #[test]
    fn no_tool_next_to_the_test_binary() {
        assert!(tool().is_none());
    }
}
