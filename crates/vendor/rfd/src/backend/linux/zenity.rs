use std::{
    error::Error,
    fmt::Display,
    path::PathBuf,
    process::{Command, ExitStatus, Output},
};

use crate::{
    message_dialog::{MessageButtons, MessageLevel},
    FileDialog, MessageDialogResult,
};

#[derive(Debug)]
pub enum ZenityError {
    Io(std::io::Error),
    FromUtf8Error(std::string::FromUtf8Error),
    Failed(ExitStatus),
    NoLocalPath,
}

impl Error for ZenityError {}

impl Display for ZenityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZenityError::Io(io) => write!(f, "{io}"),
            ZenityError::FromUtf8Error(err) => err.fmt(f),
            ZenityError::Failed(status) => write!(f, "file chooser exited with {status}"),
            ZenityError::NoLocalPath => f.write_str("file chooser returned no local file path"),
        }
    }
}

impl From<std::io::Error> for ZenityError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::string::FromUtf8Error> for ZenityError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::FromUtf8Error(value)
    }
}

pub type ZenityResult<T> = Result<T, ZenityError>;

fn command() -> Command {
    let mut cmd = Command::new("zenity");
    cmd.arg("--no-markup");
    cmd
}

fn file_dialog_command(dialog: &FileDialog) -> Command {
    let mut command = command();

    for filter in &dialog.filters {
        command.arg("--file-filter");
        let extensions: Vec<_> = filter
            .extensions
            .iter()
            .map(|ext| {
                if ext == "*" || ext.is_empty() {
                    "*".to_owned()
                } else {
                    format!("*.{}", ext)
                }
            })
            .collect();
        command.arg(format!("{} | {}", filter.name, extensions.join(" ")));
    }

    if let Some(name) = dialog.file_name.as_ref() {
        command.arg("--filename");
        command.arg(name);
    }

    if let Some(dir) = dialog.starting_directory.as_ref() {
        command.current_dir(dir);
    }

    command
}

async fn output(mut command: Command) -> ZenityResult<Output> {
    let (tx, rx) = crate::oneshot::channel();
    std::thread::spawn(move || {
        tx.send(command.output()).ok();
    });
    Ok(rx.await.map_err(std::io::Error::other)??)
}

async fn run(command: Command) -> ZenityResult<Option<String>> {
    let res = output(command).await?;
    let buffer = String::from_utf8(res.stdout)?;

    Ok((res.status.success() || !buffer.is_empty()).then_some(buffer))
}

async fn run_path(mut command: Command) -> ZenityResult<Option<PathBuf>> {
    // Keep the file picker exit protocol stable even if the session overrides it.
    command.envs([
        ("ZENITY_OK", "0"),
        ("ZENITY_CANCEL", "1"),
        ("ZENITY_ESC", "1"),
        ("ZENITY_ERROR", "255"),
    ]);
    let res = output(command).await?;
    match res.status.code() {
        Some(0) => {
            let buffer = String::from_utf8(res.stdout)?;
            let path = PathBuf::from(strip_newline(&buffer));
            if path.is_absolute() {
                Ok(Some(path))
            } else {
                Err(ZenityError::NoLocalPath)
            }
        }
        Some(1) => Ok(None),
        _ => Err(ZenityError::Failed(res.status)),
    }
}

/// Strip the single newline zenity prints after its output
fn strip_newline(buffer: &str) -> &str {
    buffer.strip_suffix('\n').unwrap_or(buffer)
}

pub async fn pick_file(dialog: &FileDialog) -> ZenityResult<Option<PathBuf>> {
    let mut command = file_dialog_command(dialog);
    command.arg("--file-selection");

    run_path(command).await
}

pub async fn pick_files(dialog: &FileDialog) -> ZenityResult<Vec<PathBuf>> {
    let mut command = file_dialog_command(dialog);
    command.args(["--file-selection", "--multiple", "--separator=\n"]);

    Ok(run(command)
        .await?
        .as_deref()
        .map(strip_newline)
        .map(|s| s.split('\n').map(PathBuf::from).collect())
        .unwrap_or_default())
}

pub async fn pick_folder(dialog: &FileDialog) -> ZenityResult<Option<PathBuf>> {
    let mut command = file_dialog_command(dialog);
    command.args(["--file-selection", "--directory"]);

    run_path(command).await
}

pub async fn pick_folders(dialog: &FileDialog) -> ZenityResult<Vec<PathBuf>> {
    let mut command = file_dialog_command(dialog);
    command.args([
        "--file-selection",
        "--directory",
        "--multiple",
        "--separator=\n",
    ]);

    Ok(run(command)
        .await?
        .as_deref()
        .map(strip_newline)
        .map(|s| s.split('\n').map(PathBuf::from).collect())
        .unwrap_or_default())
}

pub async fn save_file(dialog: &FileDialog) -> ZenityResult<Option<PathBuf>> {
    let mut command = file_dialog_command(dialog);
    command.args(["--file-selection", "--save", "--confirm-overwrite"]);

    run_path(command).await
}

pub async fn message(
    level: &MessageLevel,
    btns: &MessageButtons,
    title: &str,
    description: &str,
) -> ZenityResult<MessageDialogResult> {
    let cmd = match level {
        MessageLevel::Info => "--info",
        MessageLevel::Warning => "--warning",
        MessageLevel::Error => "--error",
    };

    let ok_label = match btns {
        MessageButtons::Ok => None,
        MessageButtons::OkCustom(ok) => Some(ok),
        _ => None,
    };

    let mut command = command();
    command.args([cmd, "--title", title, "--text", description]);

    if let Some(ok) = ok_label {
        command.args(["--ok-label", ok]);
    }

    run(command).await.map(|res| match res {
        Some(_) => match btns {
            MessageButtons::OkCustom(ok) => MessageDialogResult::Custom(ok.clone()),
            _ => MessageDialogResult::Ok,
        },
        None => MessageDialogResult::Cancel,
    })
}

pub async fn question(
    btns: &MessageButtons,
    title: &str,
    description: &str,
) -> ZenityResult<MessageDialogResult> {
    let mut command = command();
    command.args(["--question", "--title", title, "--text", description]);

    match btns {
        MessageButtons::OkCancel => {
            command.args(["--ok-label", "Ok"]);
            command.args(["--cancel-label", "Cancel"]);
        }
        MessageButtons::OkCancelCustom(ok, cancel) => {
            command.args(["--ok-label", ok.as_str()]);
            command.args(["--cancel-label", cancel.as_str()]);
        }
        MessageButtons::YesNoCancel => {
            command.args(["--extra-button", "No"]);
            command.args(["--cancel-label", "Cancel"]);
        }
        MessageButtons::YesNoCancelCustom(yes, no, cancel) => {
            command.args(["--ok-label", yes.as_str()]);
            command.args(["--cancel-label", cancel.as_str()]);
            command.args(["--extra-button", no.as_str()]);
        }
        _ => {}
    }

    run(command).await.map(|res| match btns {
        MessageButtons::OkCancel => match res {
            Some(_) => MessageDialogResult::Ok,
            None => MessageDialogResult::Cancel,
        },
        MessageButtons::YesNo => match res {
            Some(_) => MessageDialogResult::Yes,
            None => MessageDialogResult::No,
        },
        MessageButtons::OkCancelCustom(ok, cancel) => match res {
            Some(_) => MessageDialogResult::Custom(ok.clone()),
            None => MessageDialogResult::Custom(cancel.clone()),
        },
        MessageButtons::YesNoCancel => match res {
            Some(output) if output.is_empty() => MessageDialogResult::Yes,
            Some(_) => MessageDialogResult::No,
            None => MessageDialogResult::Cancel,
        },
        MessageButtons::YesNoCancelCustom(yes, no, cancel) => match res {
            Some(output) if output.is_empty() => MessageDialogResult::Custom(yes.clone()),
            Some(_) => MessageDialogResult::Custom(no.clone()),
            None => MessageDialogResult::Custom(cancel.clone()),
        },
        _ => MessageDialogResult::Cancel,
    })
}

#[cfg(test)]
mod tests {
    use crate::FileDialog;

    #[test]
    #[ignore]
    fn message() {
        pollster::block_on(super::message(
            &crate::message_dialog::MessageLevel::Info,
            &crate::message_dialog::MessageButtons::Ok,
            "hi",
            "me",
        ))
        .unwrap();
        pollster::block_on(super::message(
            &crate::message_dialog::MessageLevel::Warning,
            &crate::message_dialog::MessageButtons::Ok,
            "hi",
            "me",
        ))
        .unwrap();
        pollster::block_on(super::message(
            &crate::message_dialog::MessageLevel::Error,
            &crate::message_dialog::MessageButtons::Ok,
            "hi",
            "me",
        ))
        .unwrap();
    }

    #[test]
    #[ignore]
    fn question() {
        pollster::block_on(super::question(
            &crate::message_dialog::MessageButtons::OkCancel,
            "hi",
            "me",
        ))
        .unwrap();
        pollster::block_on(super::question(
            &crate::message_dialog::MessageButtons::YesNo,
            "hi",
            "me",
        ))
        .unwrap();
    }

    #[test]
    #[ignore]
    fn pick_file() {
        let path = pollster::block_on(super::pick_file(&FileDialog::default())).unwrap();
        dbg!(path);
    }

    #[test]
    #[ignore]
    fn pick_files() {
        let path = pollster::block_on(super::pick_files(&FileDialog::default())).unwrap();
        dbg!(path);
    }

    #[test]
    #[ignore]
    fn pick_folder() {
        let path = pollster::block_on(super::pick_folder(&FileDialog::default())).unwrap();
        dbg!(path);
    }

    #[test]
    #[ignore]
    fn save_file() {
        let path = pollster::block_on(super::save_file(&FileDialog::default())).unwrap();
        dbg!(path);
    }
}
