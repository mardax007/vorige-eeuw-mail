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

/// The names inside the scratch folder. `readpst` opens files through the ANSI code page, so a
/// path with a letter outside it (a user folder "Łucja") would fail; it runs inside the scratch
/// folder and gets only these plain names.
const INPUT: &str = "in.pst";
pub const OUTPUT: &str = "out";

/// The arguments: folders as on disk (`-r`), one `.eml` per message (`-e`), only mail (`-t e`),
/// no RTF copies of the text as attachments (`-b`), quiet (`-q`).
fn args() -> [&'static str; 9] {
    ["-r", "-e", "-q", "-b", "-t", "e", "-o", OUTPUT, INPUT]
}

/// Run the tool on `pst` (our own copy) inside the new scratch folder `work`; the messages land
/// in `work\out`. The copy is hard-linked in (no second copy of a big file), copied only when
/// the disk cannot link.
pub fn convert(tool: &Path, pst: &Path, work: &Path) -> Result<(), String> {
    std::fs::create_dir_all(work.join(OUTPUT)).map_err(|e| format!("rescue folder: {}", e.kind()))?;
    let input = work.join(INPUT);
    if std::fs::hard_link(pst, &input).is_err() {
        std::fs::copy(pst, &input).map_err(|e| format!("rescue copy: {}", e.kind()))?;
    }
    let mut cmd = Command::new(tool);
    cmd.args(args()).current_dir(work);
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
        assert_eq!(args(), ["-r", "-e", "-q", "-b", "-t", "e", "-o", "out", "in.pst"]);
    }

    #[test]
    fn no_tool_next_to_the_test_binary() {
        assert!(tool().is_none());
    }

    /// The real `readpst`, when `VEM_READPST` names it (CI sets it after installing MSYS2's
    /// libpst), on a scratch folder whose name `readpst` cannot open through the ANSI code page.
    #[test]
    fn real_readpst_works_under_a_folder_outside_the_code_page() {
        let Some(tool) = std::env::var_os("VEM_READPST").map(std::path::PathBuf::from) else {
            eprintln!("note: VEM_READPST not set, the real readpst is not tried");
            return;
        };
        let d = tempfile::tempdir().unwrap();
        let pst = d.path().join("Łucja.pst");
        std::fs::copy(crate::import::pst::tests::empty_pst(), &pst).unwrap();
        let before = std::fs::read(&pst).unwrap();
        let work = d.path().join("Łucja").join("j");
        convert(&tool, &pst, &work).unwrap();
        assert!(work.join(OUTPUT).join("Empty").is_dir(), "the root folder of Empty.pst is written");
        assert_eq!(std::fs::read(&pst).unwrap(), before, "our copy is not changed");
    }
}
