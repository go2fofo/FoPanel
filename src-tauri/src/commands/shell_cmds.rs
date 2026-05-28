use serde::Serialize;
use std::{
  path::PathBuf,
  process::Command,
};

#[cfg(windows)]
use std::env;

#[derive(Debug, Clone, Serialize)]
pub struct TerminalApp {
  pub id: String,
  pub label: String,
  pub installed: bool,
}

#[tauri::command]
pub fn shell_exec(program: String, args: Vec<String>) -> Result<String, String> {
  let out = Command::new(&program)
    .args(args)
    .output()
    .map_err(|e| e.to_string())?;
  let mut text = String::new();
  if !out.stdout.is_empty() {
    text.push_str(&String::from_utf8_lossy(&out.stdout));
  }
  if !out.stderr.is_empty() {
    if !text.is_empty() {
      text.push('\n');
    }
    text.push_str(&String::from_utf8_lossy(&out.stderr));
  }
  Ok(text.trim().to_string())
}

#[tauri::command]
pub fn list_terminal_apps() -> Result<Vec<TerminalApp>, String> {
  let mut out = Vec::<TerminalApp>::new();

  #[cfg(target_os = "macos")]
  {
    out.push(TerminalApp {
      id: "mac-terminal".to_string(),
      label: "Terminal（系统）".to_string(),
      installed: true,
    });
    out.push(TerminalApp {
      id: "mac-iterm2".to_string(),
      label: "iTerm2".to_string(),
      installed: mac_app_exists(&["/Applications/iTerm.app", "/Applications/iTerm2.app"]),
    });
    out.push(TerminalApp {
      id: "mac-warp".to_string(),
      label: "Warp".to_string(),
      installed: mac_app_exists(&["/Applications/Warp.app"]),
    });
  }

  #[cfg(windows)]
  {
    out.push(TerminalApp {
      id: "win-terminal".to_string(),
      label: "Windows Terminal".to_string(),
      installed: find_win_terminal().is_some(),
    });
    out.push(TerminalApp {
      id: "win-pwsh".to_string(),
      label: "PowerShell（pwsh）".to_string(),
      installed: find_win_pwsh().is_some(),
    });
    out.push(TerminalApp {
      id: "win-powershell".to_string(),
      label: "PowerShell（系统）".to_string(),
      installed: find_win_powershell().is_some(),
    });
    out.push(TerminalApp {
      id: "win-cmd".to_string(),
      label: "命令提示符（cmd）".to_string(),
      installed: find_win_cmd().is_some(),
    });
    out.push(TerminalApp {
      id: "win-git-bash".to_string(),
      label: "Git Bash".to_string(),
      installed: find_win_git_bash().is_some(),
    });
  }

  #[cfg(not(any(target_os = "macos", windows)))]
  {
    out.push(TerminalApp {
      id: "unknown".to_string(),
      label: "当前平台暂不支持".to_string(),
      installed: false,
    });
  }

  Ok(out)
}

#[tauri::command]
pub fn open_terminal(app_id: String, language: String, dir: Option<String>) -> Result<(), String> {
  let dir = dir.unwrap_or_else(|| "".to_string());
  let dir = dir.trim().to_string();
  if !dir.is_empty() {
    let p = PathBuf::from(&dir);
    if !p.exists() || !p.is_dir() {
      return Err("目录不存在".to_string());
    }
  }
  let cmd = version_command(&language).ok_or_else(|| "不支持的语言".to_string())?;

  #[cfg(target_os = "macos")]
  {
    return open_terminal_macos(&app_id, &cmd, if dir.is_empty() { None } else { Some(dir) });
  }
  #[cfg(windows)]
  {
    return open_terminal_windows(&app_id, &cmd, if dir.is_empty() { None } else { Some(dir) });
  }
  #[cfg(not(any(target_os = "macos", windows)))]
  {
    let _ = app_id;
    return Err("当前平台暂不支持打开终端".to_string());
  }
}

fn version_command(language: &str) -> Option<String> {
  match language {
    "node" => Some("node -v".to_string()),
    "python" => Some("python --version".to_string()),
    "bun" => Some("bun -v".to_string()),
    "deno" => Some("deno --version".to_string()),
    "go" => Some("go version".to_string()),
    "rust" => Some("rustc --version".to_string()),
    "php" => Some("php -v".to_string()),
    "java" => Some("java -version".to_string()),
    _ => None,
  }
}

#[cfg(target_os = "macos")]
fn mac_app_exists(paths: &[&str]) -> bool {
  paths.iter().any(|p| PathBuf::from(p).exists())
}

#[cfg(target_os = "macos")]
fn open_terminal_macos(app_id: &str, command: &str, dir: Option<String>) -> Result<(), String> {
  let d = dir.unwrap_or_else(|| "~".to_string());
  match app_id {
    "mac-terminal" => {
      let line = format!(
        "cd {}; {}",
        apple_script_escape_for_double_quotes(&d),
        apple_script_escape_for_double_quotes(command)
      );
      let script = vec![
        "tell application \"Terminal\"".to_string(),
        "activate".to_string(),
        format!(
          "do script \"{}\"",
          apple_script_escape_for_double_quotes(&line)
        ),
        "end tell".to_string(),
      ];
      let mut cmd = Command::new("osascript");
      for line in script {
        cmd.args(["-e", &line]);
      }
      cmd.spawn().map_err(|e| e.to_string())?;
      Ok(())
    }
    "mac-iterm2" => {
      if !mac_app_exists(&["/Applications/iTerm.app", "/Applications/iTerm2.app"]) {
        return Err("未检测到 iTerm2".to_string());
      }
      let line = format!(
        "cd {}; {}",
        apple_script_escape_for_double_quotes(&d),
        apple_script_escape_for_double_quotes(command)
      );
      let script = vec![
        "tell application \"iTerm2\"".to_string(),
        "create window with default profile".to_string(),
        format!(
          "tell current session of current window to write text \"{}\"",
          apple_script_escape_for_double_quotes(&line)
        ),
        "end tell".to_string(),
      ];
      let mut cmd = Command::new("osascript");
      for line in script {
        cmd.args(["-e", &line]);
      }
      cmd.spawn().map_err(|e| e.to_string())?;
      Ok(())
    }
    "mac-warp" => {
      if !mac_app_exists(&["/Applications/Warp.app"]) {
        return Err("未检测到 Warp".to_string());
      }
      Command::new("open")
        .args(["-a", "Warp"])
        .spawn()
        .map_err(|e| e.to_string())?;
      let line = format!(
        "cd {}; {}",
        apple_script_escape_for_double_quotes(&d),
        apple_script_escape_for_double_quotes(command)
      );
      let script = vec![
        "tell application \"Warp\" to activate".to_string(),
        "delay 0.2".to_string(),
        "tell application \"System Events\" to tell process \"Warp\"".to_string(),
        format!(
          "keystroke \"{}\"",
          apple_script_escape_for_double_quotes(&line)
        ),
        "key code 36".to_string(),
        "end tell".to_string(),
      ];
      let mut cmd = Command::new("osascript");
      for line in script {
        cmd.args(["-e", &line]);
      }
      let _ = cmd.spawn();
      Ok(())
    }
    _ => Err("不支持的终端类型".to_string()),
  }
}

#[cfg(target_os = "macos")]
fn apple_script_escape_for_double_quotes(s: &str) -> String {
  let mut out = String::new();
  for ch in s.chars() {
    if ch == '\\' {
      out.push_str("\\\\");
    } else if ch == '"' {
      out.push_str("\\\"");
    } else {
      out.push(ch);
    }
  }
  out
}

#[cfg(windows)]
fn find_in_path(program: &str) -> Option<PathBuf> {
  let path = env::var_os("PATH")?;
  let exts = env::var_os("PATHEXT")
    .map(|x| x.to_string_lossy().to_string())
    .unwrap_or_else(|| ".EXE;.COM".to_string());
  let exts = exts
    .split(';')
    .map(|x| x.trim().to_string())
    .filter(|x| !x.is_empty())
    .collect::<Vec<_>>();
  for dir in env::split_paths(&path) {
    let raw = dir.join(program);
    for ext in &exts {
      let candidate = PathBuf::from(format!("{}{}", raw.to_string_lossy(), ext));
      if candidate.exists() && candidate.is_file() {
        return Some(candidate);
      }
      let candidate = PathBuf::from(format!("{}{}", raw.to_string_lossy(), ext.to_lowercase()));
      if candidate.exists() && candidate.is_file() {
        return Some(candidate);
      }
    }
  }
  None
}

#[cfg(windows)]
fn find_win_cmd() -> Option<PathBuf> {
  find_in_path("cmd").or_else(|| Some(PathBuf::from(r"C:\Windows\System32\cmd.exe")).filter(|p| p.exists()))
}

#[cfg(windows)]
fn find_win_powershell() -> Option<PathBuf> {
  find_in_path("powershell").or_else(|| {
    Some(PathBuf::from(
      r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
    ))
    .filter(|p| p.exists())
  })
}

#[cfg(windows)]
fn find_win_pwsh() -> Option<PathBuf> {
  if let Some(p) = find_in_path("pwsh") {
    return Some(p);
  }
  if let Some(pf) = env::var_os("ProgramFiles") {
    let p = PathBuf::from(pf).join("PowerShell").join("7").join("pwsh.exe");
    if p.exists() {
      return Some(p);
    }
    let p = PathBuf::from(pf)
      .join("PowerShell")
      .join("7-preview")
      .join("pwsh.exe");
    if p.exists() {
      return Some(p);
    }
  }
  None
}

#[cfg(windows)]
fn find_win_terminal() -> Option<PathBuf> {
  if let Some(p) = find_in_path("wt") {
    return Some(p);
  }
  if let Some(local) = env::var_os("LOCALAPPDATA") {
    let p = PathBuf::from(local)
      .join("Microsoft")
      .join("WindowsApps")
      .join("wt.exe");
    if p.exists() {
      return Some(p);
    }
  }
  None
}

#[cfg(windows)]
fn find_win_git_bash() -> Option<PathBuf> {
  if let Some(pf) = env::var_os("ProgramFiles") {
    let p = PathBuf::from(pf).join("Git").join("git-bash.exe");
    if p.exists() {
      return Some(p);
    }
  }
  if let Some(pf) = env::var_os("ProgramFiles(x86)") {
    let p = PathBuf::from(pf).join("Git").join("git-bash.exe");
    if p.exists() {
      return Some(p);
    }
  }
  None
}

#[cfg(windows)]
fn open_terminal_windows(app_id: &str, command: &str, dir: Option<String>) -> Result<(), String> {
  let dir = dir.unwrap_or_else(|| env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string()));
  let d = dir.replace('"', "");
  match app_id {
    "win-terminal" => {
      let wt = find_win_terminal().ok_or_else(|| "未检测到 Windows Terminal（wt.exe）".to_string())?;
      Command::new("cmd")
        .args([
          "/C",
          "start",
          "",
          &wt.to_string_lossy(),
          "-d",
          &d,
          "cmd",
          "/K",
          command,
        ])
        .spawn()
        .map_err(|e| e.to_string())?;
      Ok(())
    }
    "win-pwsh" => {
      let pwsh = find_win_pwsh().ok_or_else(|| "未检测到 pwsh.exe".to_string())?;
      Command::new("cmd")
        .args([
          "/C",
          "start",
          "",
          &pwsh.to_string_lossy(),
          "-NoExit",
          "-Command",
          &format!("Set-Location -LiteralPath \"{}\"; {}", d, command),
        ])
        .spawn()
        .map_err(|e| e.to_string())?;
      Ok(())
    }
    "win-powershell" => {
      let ps = find_win_powershell().ok_or_else(|| "未检测到 powershell.exe".to_string())?;
      Command::new("cmd")
        .args([
          "/C",
          "start",
          "",
          &ps.to_string_lossy(),
          "-NoExit",
          "-Command",
          &format!("Set-Location -LiteralPath \"{}\"; {}", d, command),
        ])
        .spawn()
        .map_err(|e| e.to_string())?;
      Ok(())
    }
    "win-cmd" => {
      let cmd_exe = find_win_cmd().ok_or_else(|| "未检测到 cmd.exe".to_string())?;
      Command::new("cmd")
        .args([
          "/C",
          "start",
          "",
          &cmd_exe.to_string_lossy(),
          "/K",
          &format!("cd /d \"{}\" && {}", d, command),
        ])
        .spawn()
        .map_err(|e| e.to_string())?;
      Ok(())
    }
    "win-git-bash" => {
      let bash = find_win_git_bash().ok_or_else(|| "未检测到 Git Bash".to_string())?;
      let script = format!("{}; exec bash", command);
      Command::new("cmd")
        .args([
          "/C",
          "start",
          "",
          &bash.to_string_lossy(),
          &format!("--cd={}", d),
          "-c",
          &script,
        ])
        .spawn()
        .map_err(|e| e.to_string())?;
      Ok(())
    }
    _ => Err("不支持的终端类型".to_string()),
  }
}

#[tauri::command]
pub fn _legacy_shell_exec(_program: String, _args: Vec<String>) -> Result<String, String> {
  Err("已废弃".to_string())
}
