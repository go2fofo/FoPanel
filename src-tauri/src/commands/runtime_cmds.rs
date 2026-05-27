/*
 * @Author: fofo
 * @Date: 2026-05-26 15:54:28
 * @LastEditTime: 2026-05-26 16:16:45
 * @LastEditors: fofo
 * @Description: 
 * @FilePath: /FoPanel/src-tauri/src/commands/runtime_cmds.rs
 */
use crate::{models::runtime::RuntimeVersion, services::runtime_service};
use serde::{Deserialize, Serialize};
use std::{
  collections::{HashMap, HashSet},
  env,
  fs,
  path::{Path, PathBuf},
  process::Command,
};
use tauri::AppHandle;
use tauri::Manager;

#[tauri::command]
pub fn ping() -> &'static str {
  "pong"
}

#[tauri::command]
pub fn scan_runtimes(app: AppHandle) -> Result<Vec<RuntimeVersion>, String> {
  let mut list = runtime_service::scan_system_runtimes();
  list.extend(load_manual(&app)?);

  let shims = shims_dir(&app)?;
  let mut sys = Vec::<SystemRuntime>::new();
  sys.extend(system_detect_node(&shims)?);
  sys.extend(system_detect_python(&shims)?);
  sys.extend(system_detect_java(&shims)?);
  sys.extend(system_detect_simple(&shims, "bun", &["--version"])?);
  sys.extend(system_detect_deno(&shims)?);
  sys.extend(system_detect_simple(&shims, "go", &["version"])?);
  sys.extend(system_detect_simple(&shims, "php", &["-v"])?);
  sys.extend(system_detect_simple(&shims, "rustc", &["--version"])?);

  let mut sys_map = HashMap::<String, SystemRuntime>::new();
  for r in sys {
    sys_map.insert(r.language.clone(), r);
  }

  for item in list.iter_mut() {
    let Some(sel) = sys_map.get(&item.language) else {
      item.active = false;
      continue;
    };
    if !item.path.is_empty() && item.path == sel.path {
      item.active = true;
      continue;
    }
    item.active = item.version == sel.version && item.source == sel.source;
  }

  let mut seen = HashSet::<(String, String, String, String)>::new();
  list.retain(|r| {
    seen.insert((
      r.language.clone(),
      r.version.clone(),
      r.path.clone(),
      r.source.clone(),
    ))
  });
  list.sort_by(|a, b| a.language.cmp(&b.language).then(a.version.cmp(&b.version)));
  Ok(list)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRuntime {
  pub language: String,
  pub version: String,
  pub path: String,
  pub source: String,
  pub using_fopanel: bool,
}

#[tauri::command]
pub fn get_system_runtimes(app: AppHandle) -> Result<Vec<SystemRuntime>, String> {
  let shims = shims_dir(&app)?;
  let mut out = Vec::<SystemRuntime>::new();
  out.extend(system_detect_node(&shims)?);
  out.extend(system_detect_python(&shims)?);
  out.extend(system_detect_java(&shims)?);
  out.extend(system_detect_simple(&shims, "bun", &["--version"])?);
  out.extend(system_detect_deno(&shims)?);
  out.extend(system_detect_simple(&shims, "go", &["version"])?);
  out.extend(system_detect_simple(&shims, "php", &["-v"])?);
  out.extend(system_detect_simple(&shims, "rustc", &["--version"])?);
  Ok(out)
}

#[tauri::command]
pub fn get_activated_runtimes(app: AppHandle) -> Result<Vec<SystemRuntime>, String> {
  let shims = shims_dir(&app)?;
  let mut out = Vec::<SystemRuntime>::new();
  out.extend(system_detect_node_activated(&shims)?);
  out.extend(system_detect_python_activated(&shims)?);
  Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateRuntimeInput {
  pub language: String,
  pub version: String,
  pub path: String,
  pub source: String,
}

#[tauri::command]
pub fn remove_runtime(app: AppHandle, runtime: ActivateRuntimeInput) -> Result<(), String> {
  if runtime.source == "manual" {
    let mut list = load_manual(&app)?;
    list.retain(|r| !is_same_runtime(r, &runtime));
    save_manual(&app, &list)?;
    cleanup_active_if_needed(&app, &runtime)?;
    return Ok(());
  }

  Err("已取消隐藏功能：系统扫描到的版本不支持从列表移除，可使用“卸载”或在系统中自行处理。".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerOption {
  pub id: String,
  pub label: String,
  pub description: String,
}

#[tauri::command]
pub fn list_installers(app: AppHandle, language: String) -> Result<Vec<InstallerOption>, String> {
  let mut out = Vec::<InstallerOption>::new();
  let overrides = load_installer_overrides(&app)?;

  match language.as_str() {
    "node" => {
      if has_command("fnm") {
        out.push(InstallerOption {
          id: "fnm".to_string(),
          label: "fnm".to_string(),
          description: "通过 fnm 安装与管理 Node 版本。".to_string(),
        });
      }
      if (cfg!(windows) && nvm_exe_path(&overrides).is_some()) || (!cfg!(windows) && nvm_sh_exists()) {
        out.push(InstallerOption {
          id: "nvm".to_string(),
          label: "nvm".to_string(),
          description: "通过 nvm 安装与管理 Node 版本。".to_string(),
        });
      }
    }
    "python" => {
      if has_command("pyenv") {
        out.push(InstallerOption {
          id: "pyenv".to_string(),
          label: "pyenv".to_string(),
          description: "通过 pyenv 安装与管理 Python 版本。".to_string(),
        });
      }
    }
    "rust" => {
      if has_command("rustup") {
        out.push(InstallerOption {
          id: "rustup".to_string(),
          label: "rustup".to_string(),
          description: "通过 rustup 安装与管理 Rust toolchain。".to_string(),
        });
      }
    }
    "go" => {
      if has_command("goenv") {
        out.push(InstallerOption {
          id: "goenv".to_string(),
          label: "goenv".to_string(),
          description: "通过 goenv 安装与管理 Go 版本。".to_string(),
        });
      }
      if cfg!(windows) {
        if has_command("winget") {
          out.push(InstallerOption {
            id: "winget".to_string(),
            label: "winget".to_string(),
            description: "通过 winget 安装与管理该语言版本。".to_string(),
          });
        }
      } else if has_command("brew") {
        out.push(InstallerOption {
          id: "homebrew".to_string(),
          label: "Homebrew".to_string(),
          description: "通过 Homebrew 安装与管理该语言版本。".to_string(),
        });
      }
    }
    "php" => {
      if has_command("phpenv") || overrides.get("phpenv").is_some() {
        out.push(InstallerOption {
          id: "phpenv".to_string(),
          label: "phpenv".to_string(),
          description: "通过 phpenv 安装与管理 PHP 版本。".to_string(),
        });
      }
      if cfg!(windows) {
        if has_command("winget") {
          out.push(InstallerOption {
            id: "winget".to_string(),
            label: "winget".to_string(),
            description: "通过 winget 安装与管理该语言版本。".to_string(),
          });
        }
      } else if has_command("brew") {
        out.push(InstallerOption {
          id: "homebrew".to_string(),
          label: "Homebrew".to_string(),
          description: "通过 Homebrew 安装与管理该语言版本。".to_string(),
        });
      }
    }
    "java" => {
      if !cfg!(windows) && (sdkman_is_available() || overrides.get("sdkman").is_some()) {
        out.push(InstallerOption {
          id: "sdkman".to_string(),
          label: "SDKMAN".to_string(),
          description: "通过 SDKMAN 安装与管理 Java（适用于 macOS/Linux）。".to_string(),
        });
      }
      if cfg!(windows) {
        if has_command("winget") {
          out.push(InstallerOption {
            id: "winget".to_string(),
            label: "winget".to_string(),
            description: "通过 winget 安装与管理该语言版本。".to_string(),
          });
        }
      } else if has_command("brew") {
        out.push(InstallerOption {
          id: "homebrew".to_string(),
          label: "Homebrew".to_string(),
          description: "通过 Homebrew 安装与管理该语言版本。".to_string(),
        });
      }
    }
    "bun" | "deno" => {
      if cfg!(windows) {
        if has_command("winget") {
          out.push(InstallerOption {
            id: "winget".to_string(),
            label: "winget".to_string(),
            description: "通过 winget 安装与管理该语言版本。".to_string(),
          });
        }
      } else if has_command("brew") {
        out.push(InstallerOption {
          id: "homebrew".to_string(),
          label: "Homebrew".to_string(),
          description: "通过 Homebrew 安装与管理该语言版本。".to_string(),
        });
      }
    }
    _ => {}
  }

  Ok(out)
}

#[tauri::command]
pub fn install_runtime(
  app: AppHandle,
  language: String,
  installer: String,
  version: String,
) -> Result<String, String> {
  let overrides = load_installer_overrides(&app)?;
  let output = match (language.as_str(), installer.as_str()) {
    ("node", "fnm") => run_install_fnm(&version)?,
    ("node", "nvm") => run_install_nvm(&version, &overrides)?,
    ("python", "pyenv") => run_install_pyenv(&version)?,
    ("rust", "rustup") => run_install_rustup(&version)?,
    ("bun", "homebrew") => run_install_homebrew("oven-sh/bun/bun", &version)?,
    ("deno", "homebrew") => run_install_homebrew("deno", &version)?,
    ("go", "homebrew") => run_install_homebrew("go", &version)?,
    ("go", "goenv") => run_install_goenv(&version)?,
    ("php", "phpenv") => run_install_phpenv(&version, &overrides)?,
    ("php", "homebrew") => run_install_homebrew(&homebrew_php_formula(&version), &version)?,
    ("java", "homebrew") => run_install_homebrew(&homebrew_java_formula(&version), &version)?,
    ("java", "sdkman") => run_install_sdkman_java(&version, &overrides)?,
    ("bun", "winget") => run_install_winget("Oven-sh.Bun", &version)?,
    ("deno", "winget") => run_install_winget("DenoLand.Deno", &version)?,
    ("go", "winget") => run_install_winget("GoLang.Go", &version)?,
    ("php", "winget") => run_install_winget(&winget_php_id(&version), &version)?,
    ("java", "winget") => {
      let major = java_major_from_version(&version).unwrap_or(21);
      run_install_winget(&winget_java_id(major), "latest")?
    }
    _ => return Err("暂不支持该语言/安装器组合".to_string()),
  };

  Ok(output)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProfileItem {
  pub language: String,
  pub installer: String,
  pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProfile {
  pub id: String,
  pub name: String,
  pub items: Vec<RuntimeProfileItem>,
}

#[tauri::command]
pub fn list_runtime_profiles(app: AppHandle) -> Result<Vec<RuntimeProfile>, String> {
  load_profiles(&app)
}

#[tauri::command]
pub fn upsert_runtime_profile(app: AppHandle, profile: RuntimeProfile) -> Result<(), String> {
  let mut list = load_profiles(&app)?;
  if let Some(old) = list.iter_mut().find(|p| p.id == profile.id) {
    *old = profile;
  } else {
    list.push(profile);
  }
  save_profiles(&app, &list)
}

#[tauri::command]
pub fn delete_runtime_profile(app: AppHandle, id: String) -> Result<(), String> {
  let mut list = load_profiles(&app)?;
  list.retain(|p| p.id != id);
  save_profiles(&app, &list)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerStatus {
  pub id: String,
  pub installed: bool,
  pub hint: String,
}

#[tauri::command]
pub fn get_installer_status(app: AppHandle) -> Result<Vec<InstallerStatus>, String> {
  let mut out = Vec::<InstallerStatus>::new();
  let overrides = load_installer_overrides(&app)?;

  let brew = has_command("brew");
  let winget = has_command("winget");

  out.push(InstallerStatus {
    id: "homebrew".to_string(),
    installed: brew,
    hint: installer_bootstrap_hint("homebrew", brew, winget),
  });
  out.push(InstallerStatus {
    id: "winget".to_string(),
    installed: winget,
    hint: installer_bootstrap_hint("winget", brew, winget),
  });
  out.push(InstallerStatus {
    id: "fnm".to_string(),
    installed: has_command("fnm"),
    hint: installer_bootstrap_hint("fnm", brew, winget),
  });
  out.push(InstallerStatus {
    id: "nvm".to_string(),
    installed: if cfg!(windows) {
      nvm_exe_path(&overrides).is_some()
    } else {
      nvm_sh_exists()
    },
    hint: installer_bootstrap_hint("nvm", brew, winget),
  });
  out.push(InstallerStatus {
    id: "pyenv".to_string(),
    installed: has_command("pyenv"),
    hint: installer_bootstrap_hint("pyenv", brew, winget),
  });
  out.push(InstallerStatus {
    id: "rustup".to_string(),
    installed: has_command("rustup"),
    hint: installer_bootstrap_hint("rustup", brew, winget),
  });
  out.push(InstallerStatus {
    id: "goenv".to_string(),
    installed: has_command("goenv"),
    hint: installer_bootstrap_hint("goenv", brew, winget),
  });
  out.push(InstallerStatus {
    id: "phpenv".to_string(),
    installed: has_command("phpenv") || overrides.get("phpenv").is_some(),
    hint: installer_bootstrap_hint("phpenv", brew, winget),
  });
  out.push(InstallerStatus {
    id: "sdkman".to_string(),
    installed: (!cfg!(windows) && sdkman_is_available()) || overrides.get("sdkman").is_some(),
    hint: installer_bootstrap_hint("sdkman", brew, winget),
  });

  Ok(out)
}

#[tauri::command]
pub fn get_installer_overrides(app: AppHandle) -> Result<HashMap<String, String>, String> {
  load_installer_overrides(&app)
}

#[tauri::command]
pub fn set_installer_override(app: AppHandle, installer: String, path: String) -> Result<(), String> {
  let p = PathBuf::from(path.trim());
  if !p.exists() || !p.is_file() {
    return Err("路径不存在或不是文件".to_string());
  }
  let mut map = load_installer_overrides(&app)?;
  map.insert(installer, p.to_string_lossy().to_string());
  save_installer_overrides(&app, &map)
}

#[tauri::command]
pub fn clear_installer_override(app: AppHandle, installer: String) -> Result<(), String> {
  let mut map = load_installer_overrides(&app)?;
  map.remove(&installer);
  save_installer_overrides(&app, &map)
}

#[tauri::command]
pub fn get_installer_bootstrap(installer: String) -> Result<String, String> {
  let brew = has_command("brew");
  let winget = has_command("winget");
  Ok(installer_bootstrap_hint(&installer, brew, winget))
}

#[tauri::command]
pub fn get_installer_env_config(installer: String) -> Result<String, String> {
  Ok(installer_env_config(&installer))
}

#[tauri::command]
pub fn install_installer(installer: String) -> Result<String, String> {
  let brew = has_command("brew");
  let winget = has_command("winget");
  let Some(cmd) = installer_install_command(&installer, brew, winget) else {
    return Err("该安装器不支持自动安装，请按“安装建议”在终端执行。".to_string());
  };
  run_login_shell(&cmd)
}

#[tauri::command]
pub fn uninstall_runtime(app: AppHandle, runtime: ActivateRuntimeInput) -> Result<String, String> {
  if runtime.source == "manual" {
    remove_runtime(app, runtime)?;
    return Ok("已删除".to_string());
  }

  let overrides = load_installer_overrides(&app)?;
  let output = match (runtime.language.as_str(), runtime.source.as_str()) {
    ("node", "fnm") => run_uninstall_fnm(&runtime.version)?,
    ("node", "nvm") => run_uninstall_nvm(&runtime.version, &overrides)?,
    ("python", "pyenv") => run_uninstall_pyenv(&runtime.version)?,
    ("rust", "rustup") => run_uninstall_rustup(&runtime.version)?,
    ("go", "goenv") => run_uninstall_goenv(&runtime.version)?,
    ("php", "phpenv") => run_uninstall_phpenv(&runtime.version, &overrides)?,
    ("java", "homebrew") => run_uninstall_homebrew(&homebrew_java_formula(&runtime.version))?,
    ("java", "sdkman") => run_uninstall_sdkman_java(&runtime.version, &overrides)?,
    ("java", "winget") => {
      let major = java_major_from_version(&runtime.version).unwrap_or(21);
      run_uninstall_winget(&winget_java_id(major))?
    }
    _ => return Err("该版本来源不支持卸载，请在系统中使用原安装方式处理".to_string()),
  };

  cleanup_active_if_needed(&app, &runtime)?;
  Ok(output)
}

#[tauri::command]
pub fn check_runtime_upgrade(runtime: ActivateRuntimeInput) -> Result<String, String> {
  let exec = resolve_exec_path(&runtime)?;
  if !exec.exists() {
    return Err("可执行文件不存在".to_string());
  }

  match runtime.language.as_str() {
    "deno" => {
      let out = Command::new(&exec)
        .args(["upgrade", "--dry-run"])
        .output()
        .map_err(|e| e.to_string())?;
      let raw = merge_output(&out).0;
      let text = strip_ansi(&raw);
      if text.contains("built without the \"upgrade\" feature") {
        let hint = deno_upgrade_hint(&exec);
        return Ok(format!("{text}\n\n{hint}"));
      }
      Ok(text.trim().to_string())
    }
    "bun" => Err("bun 暂不支持无副作用的检测更新，请手动运行 bun upgrade".to_string()),
    _ => Err("该语言暂不支持检测更新".to_string()),
  }
}

fn strip_ansi(input: &str) -> String {
  let mut out = String::new();
  let mut it = input.chars().peekable();
  while let Some(ch) = it.next() {
    if ch == '\u{1b}' {
      if matches!(it.peek(), Some('[')) {
        it.next();
        while let Some(x) = it.next() {
          if x == 'm' || x == 'K' {
            break;
          }
        }
        continue;
      }
      continue;
    }
    out.push(ch);
  }
  out
}

fn deno_upgrade_hint(exec: &Path) -> String {
  let p = exec.to_string_lossy();
  if p.starts_with("/opt/homebrew/") || p.starts_with("/usr/local/") {
    return "建议：看起来是 Homebrew 安装的 Deno，请运行：brew upgrade deno".to_string();
  }
  if p.contains(".asdf") {
    return "建议：看起来是 asdf 安装的 Deno，请用 asdf 的方式升级（例如重新 install 最新版本并 global）".to_string();
  }
  if p.contains(".deno") {
    return "建议：看起来是官方脚本安装的 Deno，请按官方安装方式重新安装/升级。".to_string();
  }
  "建议：该 Deno 禁用了自升级，请使用你最初的安装方式升级（brew/apt/scoop/asdf 等）。".to_string()
}

fn is_using_fopanel(shims: &Path, cmd_path: &str) -> bool {
  let p = PathBuf::from(cmd_path.trim());
  let real = fs::canonicalize(&p).unwrap_or(p);
  real.starts_with(shims)
}

fn classify_system_source(path: &str) -> String {
  let s = path.replace('\\', "/");
  let s = s.to_lowercase();
  if s.contains("/.nvm/versions/node/") {
    return "nvm".to_string();
  }
  if cfg!(windows) {
    if let Ok(link) = env::var("NVM_SYMLINK") {
      let link = link.replace('\\', "/").to_lowercase();
      if !link.is_empty() && s.starts_with(&link) {
        return "nvm".to_string();
      }
    }
    if s.contains("/nvm/") {
      return "nvm".to_string();
    }
  }
  if s.contains("/.fnm/") || s.contains("/fnm/") {
    return "fnm".to_string();
  }
  if s.contains("/.volta/") {
    return "volta".to_string();
  }
  if s.contains("/.asdf/") {
    return "asdf".to_string();
  }
  if s.contains("/.goenv/") {
    return "goenv".to_string();
  }
  if s.contains("/.phpenv/") {
    return "phpenv".to_string();
  }
  if s.contains("/.sdkman/") {
    return "sdkman".to_string();
  }
  if s.contains("/.pyenv/versions/") {
    return "pyenv".to_string();
  }
  if s.contains("/.pyenv/shims/") {
    return "pyenv".to_string();
  }
  if s.contains("/Cellar/") || s.starts_with("/opt/homebrew/") || s.starts_with("/usr/local/") {
    return "homebrew".to_string();
  }
  "standalone".to_string()
}

#[cfg(unix)]
fn run_login_shell(script: &str) -> Result<String, String> {
  let shells = ["zsh", "bash", "sh"];
  for shell in shells {
    let out = Command::new(shell).args(["-lic", script]).output();
    let Ok(out) = out else {
      continue;
    };
    let (text, _) = merge_output(&out);
    return Ok(strip_ansi(&text));
  }
  Err("无法执行登录 shell".to_string())
}

#[cfg(windows)]
fn run_login_shell(script: &str) -> Result<String, String> {
  let out = Command::new("powershell")
    .args(["-NoProfile", "-Command", script])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&out);
  Ok(strip_ansi(&text))
}

#[cfg(unix)]
fn run_login_shell_activated(shims: &Path, script: &str) -> Result<String, String> {
  let prelude = format!(
    "export PATH=\"{}:$PATH\"; hash -r; ",
    shims.to_string_lossy()
  );
  run_login_shell(&(prelude + script))
}

#[cfg(windows)]
fn run_login_shell_activated(shims: &Path, script: &str) -> Result<String, String> {
  let prelude = format!(
    "$env:Path = \"{};\" + $env:Path; ",
    shims.to_string_lossy()
  );
  run_login_shell(&(prelude + script))
}

fn system_detect_simple(
  shims: &Path,
  program: &str,
  args: &[&str],
) -> Result<Vec<SystemRuntime>, String> {
  let lang = if program == "rustc" { "rust" } else { program };

  #[cfg(unix)]
  let script = {
    let mut cmd = String::new();
    cmd.push_str(&format!(
      "p=$(type -P {p} 2>/dev/null); if [ -z \"$p\" ]; then exit 0; fi; echo \"$p\"; {p} ",
      p = shell_escape(program)
    ));
    for a in args {
      cmd.push_str(&format!("{} ", shell_escape(a)));
    }
    cmd
  };
  #[cfg(windows)]
  let script = {
    let mut cmd = String::new();
    cmd.push_str(&format!(
      "$p=(Get-Command {p} -ErrorAction SilentlyContinue).Source; if(!$p){{exit}}; Write-Output $p; {p} ",
      p = program
    ));
    for a in args {
      cmd.push_str(&format!("{} ", a));
    }
    cmd
  };

  let text = run_login_shell(&script)?;
  let mut lines = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty());
  let Some(cmd_path) = lines.next() else {
    return Ok(vec![]);
  };
  let ver_text = lines.collect::<Vec<_>>().join("\n");
  let ver_line = ver_text.lines().next().unwrap_or_default().trim();
  let version = if lang == "php" {
    extract_semver_like(ver_line).unwrap_or_default()
  } else if lang == "go" {
    ver_line
      .split_whitespace()
      .find(|x| x.starts_with("go"))
      .map(|x| x.trim_start_matches("go").to_string())
      .unwrap_or_default()
  } else if lang == "rust" {
    extract_semver_like(ver_line).unwrap_or_default()
  } else {
    extract_semver_like(ver_line).unwrap_or_else(|| ver_line.to_string())
  };

  let source = classify_system_source(cmd_path);
  Ok(vec![SystemRuntime {
    language: lang.to_string(),
    version,
    path: cmd_path.to_string(),
    source,
    using_fopanel: is_using_fopanel(shims, cmd_path),
  }])
}

fn system_detect_deno(shims: &Path) -> Result<Vec<SystemRuntime>, String> {
  #[cfg(unix)]
  let script = "p=$(type -P deno 2>/dev/null); if [ -z \"$p\" ]; then exit 0; fi; echo \"__FOPANEL_CMD__=$p\"; deno --version 2>/dev/null | while IFS= read -r l; do echo \"__FOPANEL_OUT__=$l\"; done";
  #[cfg(windows)]
  let script = "$p=(Get-Command deno -ErrorAction SilentlyContinue).Source; if(!$p){exit}; Write-Output \"__FOPANEL_CMD__=$p\"; $out=(deno --version 2>$null); foreach($l in $out){ Write-Output \"__FOPANEL_OUT__=$l\" }";

  let text = run_login_shell(script)?;
  let mut cmd_path = String::new();
  let mut version = String::new();
  for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
    if let Some(rest) = line.strip_prefix("__FOPANEL_CMD__=") {
      cmd_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_OUT__=") {
      let l = rest.trim();
      if l.starts_with("deno ") {
        version = extract_semver_like(l).unwrap_or_default();
      }
      continue;
    }
  }
  if cmd_path.is_empty() {
    return Ok(vec![]);
  }
  Ok(vec![SystemRuntime {
    language: "deno".to_string(),
    version,
    path: cmd_path.to_string(),
    source: classify_system_source(&cmd_path),
    using_fopanel: is_using_fopanel(shims, &cmd_path),
  }])
}

fn system_detect_node(shims: &Path) -> Result<Vec<SystemRuntime>, String> {
  #[cfg(unix)]
  let script = "p=$(type -P node 2>/dev/null); if [ -z \"$p\" ]; then exit 0; fi; echo \"__FOPANEL_CMD__=$p\"; ep=$(node -p \"process.execPath\" 2>/dev/null); if [ -n \"$ep\" ]; then echo \"__FOPANEL_REAL__=$ep\"; fi; v=$(node -v 2>/dev/null); echo \"__FOPANEL_VER__=$v\"";
  #[cfg(windows)]
  let script = "$p=(Get-Command node -ErrorAction SilentlyContinue).Source; if(!$p){exit}; Write-Output \"__FOPANEL_CMD__=$p\"; $ep=(node -p \"process.execPath\" 2>$null); if($ep){Write-Output \"__FOPANEL_REAL__=$ep\"}; $v=(node -v 2>$null); Write-Output \"__FOPANEL_VER__=$v\"";

  let text = run_login_shell(script)?;
  let mut cmd_path = String::new();
  let mut real_path = String::new();
  let mut version = String::new();
  for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
    if let Some(rest) = line.strip_prefix("__FOPANEL_CMD__=") {
      cmd_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_REAL__=") {
      real_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_VER__=") {
      let v = rest.trim();
      version = v.strip_prefix('v').unwrap_or(v).to_string();
      continue;
    }
  }
  if cmd_path.is_empty() {
    return Ok(vec![]);
  }
  if real_path.is_empty() {
    real_path = cmd_path.clone();
  }
  Ok(vec![SystemRuntime {
    language: "node".to_string(),
    version,
    path: real_path.to_string(),
    source: classify_system_source(&real_path),
    using_fopanel: is_using_fopanel(shims, &cmd_path),
  }])
}

fn system_detect_python(shims: &Path) -> Result<Vec<SystemRuntime>, String> {
  #[cfg(unix)]
  let script = "pc=$(type -P python 2>/dev/null); p3=$(type -P python3 2>/dev/null); if [ -n \"$pc\" ]; then cmd=python; p=$pc; elif [ -n \"$p3\" ]; then cmd=python3; p=$p3; else exit 0; fi; echo \"__FOPANEL_CMD__=$p\"; echo \"__FOPANEL_USED__=$cmd\"; ex=$($cmd -c \"import sys;print(sys.executable)\" 2>/dev/null); if [ -n \"$ex\" ]; then echo \"__FOPANEL_REAL__=$ex\"; fi; v=$($cmd --version 2>&1); echo \"__FOPANEL_VER__=$v\"";
  #[cfg(windows)]
  let script = "$p=(Get-Command python -ErrorAction SilentlyContinue).Source; $cmd='python'; if(!$p){$p=(Get-Command python3 -ErrorAction SilentlyContinue).Source; $cmd='python3'}; if(!$p){exit}; Write-Output \"__FOPANEL_CMD__=$p\"; Write-Output \"__FOPANEL_USED__=$cmd\"; $ex=(& $cmd -c \"import sys;print(sys.executable)\" 2>$null); if($ex){Write-Output \"__FOPANEL_REAL__=$ex\"}; $v=(& $cmd --version 2>$null); Write-Output \"__FOPANEL_VER__=$v\"";

  let text = run_login_shell(script)?;
  let mut cmd_path = String::new();
  let mut real_path = String::new();
  let mut version = String::new();
  for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
    if let Some(rest) = line.strip_prefix("__FOPANEL_CMD__=") {
      cmd_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_REAL__=") {
      real_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_VER__=") {
      let v = rest.trim();
      version = v.strip_prefix("Python ").unwrap_or(v).trim().to_string();
      continue;
    }
  }
  if cmd_path.is_empty() {
    return Ok(vec![]);
  }
  if real_path.is_empty() {
    real_path = cmd_path.clone();
  }
  Ok(vec![SystemRuntime {
    language: "python".to_string(),
    version,
    path: real_path.to_string(),
    source: classify_system_source(&real_path),
    using_fopanel: is_using_fopanel(shims, &cmd_path),
  }])
}

fn system_detect_java(shims: &Path) -> Result<Vec<SystemRuntime>, String> {
  #[cfg(unix)]
  let script =
    "p=$(type -P java 2>/dev/null); if [ -z \"$p\" ]; then exit 0; fi; echo \"__FOPANEL_CMD__=$p\"; v=$(java -version 2>&1 | head -n 1); echo \"__FOPANEL_VER__=$v\"";
  #[cfg(windows)]
  let script =
    "$p=(Get-Command java -ErrorAction SilentlyContinue).Source; if(!$p){exit}; Write-Output \"__FOPANEL_CMD__=$p\"; $v=(java -version 2>&1 | Select-Object -First 1); Write-Output \"__FOPANEL_VER__=$v\"";

  let text = run_login_shell(script)?;
  let mut cmd_path = String::new();
  let mut version = String::new();
  for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
    if let Some(rest) = line.strip_prefix("__FOPANEL_CMD__=") {
      cmd_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_VER__=") {
      let v = rest.trim();
      if v.contains("Unable to locate a Java Runtime")
        || v.contains("No Java runtime present")
        || v.contains("Unable to locate Java Runtime")
      {
        return Ok(vec![]);
      }
      version = extract_semver_like(v).unwrap_or_else(|| v.to_string());
      continue;
    }
  }
  if cmd_path.is_empty() {
    return Ok(vec![]);
  }

  let mut source = classify_system_source(&cmd_path);
  if cmd_path.contains("/.sdkman/candidates/java/") {
    source = "sdkman".to_string();
  }
  if cfg!(windows) && has_command("winget") {
    if let Some(major) = java_major_from_version(&version) {
      let id = winget_java_id(major);
      if winget_id_installed(&id) {
        source = "winget".to_string();
      }
    }
  }

  Ok(vec![SystemRuntime {
    language: "java".to_string(),
    version,
    path: cmd_path.to_string(),
    source,
    using_fopanel: is_using_fopanel(shims, &cmd_path),
  }])
}

fn system_detect_node_activated(shims: &Path) -> Result<Vec<SystemRuntime>, String> {
  #[cfg(unix)]
  let script =
    "p=$(type -P node 2>/dev/null); if [ -z \"$p\" ]; then exit 0; fi; echo \"__FOPANEL_CMD__=$p\"; ep=$(node -p \"process.execPath\" 2>/dev/null); if [ -n \"$ep\" ]; then echo \"__FOPANEL_REAL__=$ep\"; fi; v=$(node -v 2>/dev/null); echo \"__FOPANEL_VER__=$v\"";
  #[cfg(windows)]
  let script =
    "$p=(Get-Command node -ErrorAction SilentlyContinue).Source; if(!$p){exit}; Write-Output \"__FOPANEL_CMD__=$p\"; $ep=(node -p \"process.execPath\" 2>$null); if($ep){Write-Output \"__FOPANEL_REAL__=$ep\"}; $v=(node -v 2>$null); Write-Output \"__FOPANEL_VER__=$v\"";

  let text = run_login_shell_activated(shims, script)?;
  let mut cmd_path = String::new();
  let mut real_path = String::new();
  let mut version = String::new();
  for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
    if let Some(rest) = line.strip_prefix("__FOPANEL_CMD__=") {
      cmd_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_REAL__=") {
      real_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_VER__=") {
      let v = rest.trim();
      version = v.strip_prefix('v').unwrap_or(v).to_string();
      continue;
    }
  }
  if cmd_path.is_empty() {
    return Ok(vec![]);
  }
  if real_path.is_empty() {
    real_path = cmd_path.clone();
  }
  Ok(vec![SystemRuntime {
    language: "node".to_string(),
    version,
    path: real_path.to_string(),
    source: classify_system_source(&real_path),
    using_fopanel: is_using_fopanel(shims, &cmd_path),
  }])
}

fn system_detect_python_activated(shims: &Path) -> Result<Vec<SystemRuntime>, String> {
  #[cfg(unix)]
  let script =
    "pc=$(type -P python 2>/dev/null); p3=$(type -P python3 2>/dev/null); if [ -n \"$pc\" ]; then cmd=python; p=$pc; elif [ -n \"$p3\" ]; then cmd=python3; p=$p3; else exit 0; fi; echo \"__FOPANEL_CMD__=$p\"; echo \"__FOPANEL_USED__=$cmd\"; ex=$($cmd -c \"import sys;print(sys.executable)\" 2>/dev/null); if [ -n \"$ex\" ]; then echo \"__FOPANEL_REAL__=$ex\"; fi; v=$($cmd --version 2>&1); echo \"__FOPANEL_VER__=$v\"";
  #[cfg(windows)]
  let script =
    "$p=(Get-Command python -ErrorAction SilentlyContinue).Source; $cmd='python'; if(!$p){$p=(Get-Command python3 -ErrorAction SilentlyContinue).Source; $cmd='python3'}; if(!$p){exit}; Write-Output \"__FOPANEL_CMD__=$p\"; Write-Output \"__FOPANEL_USED__=$cmd\"; $ex=(& $cmd -c \"import sys;print(sys.executable)\" 2>$null); if($ex){Write-Output \"__FOPANEL_REAL__=$ex\"}; $v=(& $cmd --version 2>$null); Write-Output \"__FOPANEL_VER__=$v\"";

  let text = run_login_shell_activated(shims, script)?;
  let mut cmd_path = String::new();
  let mut real_path = String::new();
  let mut version = String::new();
  for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
    if let Some(rest) = line.strip_prefix("__FOPANEL_CMD__=") {
      cmd_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_REAL__=") {
      real_path = rest.trim().to_string();
      continue;
    }
    if let Some(rest) = line.strip_prefix("__FOPANEL_VER__=") {
      let v = rest.trim();
      version = v.strip_prefix("Python ").unwrap_or(v).trim().to_string();
      continue;
    }
  }
  if cmd_path.is_empty() {
    return Ok(vec![]);
  }
  if real_path.is_empty() {
    real_path = cmd_path.clone();
  }
  Ok(vec![SystemRuntime {
    language: "python".to_string(),
    version,
    path: real_path.to_string(),
    source: classify_system_source(&real_path),
    using_fopanel: is_using_fopanel(shims, &cmd_path),
  }])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateRuntimeResult {
  pub verified: bool,
  pub output: String,
  pub expected: String,
  pub actual: String,
}

#[tauri::command]
pub fn activate_runtime(
  app: AppHandle,
  runtime: ActivateRuntimeInput,
) -> Result<ActivateRuntimeResult, String> {
  let shims = shims_dir(&app)?;
  ensure_dir(&shims)?;

  let resolved_exec = resolve_exec_path(&runtime)?;
  if !resolved_exec.exists() {
    return Err("可执行文件不存在".to_string());
  }

  match runtime.language.as_str() {
    "node" => activate_node(&shims, &resolved_exec)?,
    "python" => activate_python(&shims, &resolved_exec)?,
    "bun" => activate_bun(&shims, &resolved_exec)?,
    "deno" => activate_simple(&shims, "deno", &resolved_exec)?,
    "go" => activate_simple(&shims, "go", &resolved_exec)?,
    "php" => activate_simple(&shims, "php", &resolved_exec)?,
    "rust" => activate_rust(&shims, &resolved_exec)?,
    _ => return Err("暂不支持该语言的激活".to_string()),
  }

  let verify = match runtime.language.as_str() {
    "node" => verify_node(&shims, &runtime.version),
    "python" => verify_python(&shims, &runtime.version),
    "bun" => verify_bun(&shims, &runtime.version),
    "deno" => verify_deno(&shims, &runtime.version),
    "go" => verify_go(&shims, &runtime.version),
    "php" => verify_php(&shims, &runtime.version),
    "rust" => verify_rust(&shims, &runtime.version),
    _ => return Err("暂不支持该语言的校验".to_string()),
  }?;

  let mut active = load_active(&app)?;
  active.insert(
    runtime.language.clone(),
    ActiveRuntime {
      version: runtime.version.clone(),
      path: resolved_exec.to_string_lossy().to_string(),
      source: runtime.source.clone(),
    },
  );
  save_active(&app, &active)?;
  Ok(verify)
}

#[tauri::command]
pub fn get_activation_export(app: AppHandle) -> Result<String, String> {
  let shims = shims_dir(&app)?;
  Ok(format!(
    "export PATH=\"{}:$PATH\"; hash -r 2>/dev/null; rehash 2>/dev/null",
    shims.to_string_lossy()
  ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePackage {
  pub name: String,
  pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDetail {
  pub runtime: RuntimeVersion,
  pub info: HashMap<String, String>,
  pub packages: Vec<RuntimePackage>,
}

#[tauri::command]
pub fn get_runtime_detail(
  app: AppHandle,
  runtime: ActivateRuntimeInput,
) -> Result<RuntimeDetail, String> {
  let resolved_exec = resolve_exec_path(&runtime)?;
  if !resolved_exec.exists() {
    return Err("可执行文件不存在".to_string());
  }

  let active = load_active(&app)?;
  let mut rt = RuntimeVersion {
    language: runtime.language.clone(),
    version: runtime.version.clone(),
    path: resolved_exec.to_string_lossy().to_string(),
    active: false,
    source: runtime.source.clone(),
  };
  if let Some(sel) = active.get(&rt.language) {
    rt.active = is_match_active(&rt, sel);
  }

  let mut info = HashMap::<String, String>::new();
  info.insert("executable".to_string(), rt.path.clone());
  info.insert("version".to_string(), rt.version.clone());
  info.insert("source".to_string(), rt.source.clone());
  let (upgrade_hint, remove_hint) = runtime_manage_hints(&rt);
  if !upgrade_hint.is_empty() {
    info.insert("升级建议".to_string(), upgrade_hint);
  }
  if !remove_hint.is_empty() {
    info.insert("卸载/隐藏".to_string(), remove_hint);
  }

  let (packages, extra) = match rt.language.as_str() {
    "python" => get_python_detail(&resolved_exec)?,
    "node" => get_node_detail(&resolved_exec)?,
    "rust" => get_rust_detail(&resolved_exec)?,
    "go" => get_go_detail(&resolved_exec)?,
    "php" => get_php_detail(&resolved_exec)?,
    "java" => get_java_detail(&resolved_exec)?,
    _ => (vec![], HashMap::new()),
  };
  info.extend(extra);

  Ok(RuntimeDetail {
    runtime: rt,
    info,
    packages,
  })
}

fn runtime_manage_hints(rt: &RuntimeVersion) -> (String, String) {
  let upgrade = match (rt.language.as_str(), rt.source.as_str()) {
    ("node", "fnm") => "多版本：fnm install <version>（安装新版本后再激活）".to_string(),
    ("node", "nvm") => "多版本：nvm install <version>（安装新版本后再激活）".to_string(),
    ("node", "volta") => "多版本：volta install node@<version>（安装新版本后再激活）".to_string(),
    ("node", "asdf") => "多版本：asdf install nodejs <version>（安装新版本后再激活）".to_string(),
    ("node", "homebrew") => "单版本：brew upgrade node（或 node@<主版本>）".to_string(),
    ("python", "pyenv") => "多版本：pyenv install -s <version>（安装新版本后再激活）".to_string(),
    ("python", "asdf") => "多版本：asdf install python <version>（安装新版本后再激活）".to_string(),
    ("python", "homebrew") => "单版本：brew upgrade python（或 python@<主版本>）".to_string(),
    ("python", "framework") => "单版本：请用系统/官方安装器升级（或通过包管理器升级）".to_string(),
    ("deno", "homebrew") => "单版本：brew upgrade deno".to_string(),
    ("bun", "homebrew") => "单版本：brew upgrade bun".to_string(),
    ("go", "goenv") => "多版本：goenv install -s <version>（安装后再切换/设置全局）".to_string(),
    ("php", "phpenv") => "多版本：phpenv install <version>（安装后再切换/设置全局）".to_string(),
    ("java", "homebrew") => "单版本：brew upgrade openjdk（或 openjdk@<主版本>）".to_string(),
    ("java", "winget") => "单版本：winget upgrade --id EclipseAdoptium.Temurin.<主版本>.JDK -e".to_string(),
    ("java", "sdkman") => "多版本：sdk install java <candidate>（例如 21.0.2-tem）".to_string(),
    ("deno", _) => "单版本：deno upgrade（若该构建启用了 upgrade 功能）".to_string(),
    ("bun", _) => "单版本：bun upgrade".to_string(),
    _ => String::new(),
  };

  let remove = match (rt.language.as_str(), rt.source.as_str()) {
    ("node", "fnm") => "卸载：fnm uninstall <version>；也可在 FoPanel 里点“卸载”".to_string(),
    ("node", "nvm") => "卸载：nvm uninstall v<version>；也可在 FoPanel 里点“卸载”".to_string(),
    ("python", "pyenv") => "卸载：pyenv uninstall <version>（可能需要 pyenv-uninstall 插件）".to_string(),
    ("rust", "rustup") => "卸载：rustup toolchain uninstall <version>；也可在 FoPanel 里点“卸载”".to_string(),
    ("go", "goenv") => "卸载：goenv uninstall <version>（可能需要 goenv-uninstall 插件）；也可在 FoPanel 里点“卸载”".to_string(),
    ("php", "phpenv") => "卸载：phpenv uninstall <version>（可能需要 phpenv-uninstall 插件）；也可在 FoPanel 里点“卸载”".to_string(),
    ("deno", "homebrew") => "卸载：brew uninstall deno；FoPanel 建议只做隐藏".to_string(),
    ("bun", "homebrew") => "卸载：brew uninstall bun；FoPanel 建议只做隐藏".to_string(),
    ("node", "homebrew") => "卸载：brew uninstall node（或 node@<主版本>）；FoPanel 建议只做隐藏".to_string(),
    ("python", "homebrew") => "卸载：brew uninstall python（或 python@<主版本>）；FoPanel 建议只做隐藏".to_string(),
    ("java", "homebrew") => "卸载：brew uninstall openjdk（或 openjdk@<主版本>）；也可在 FoPanel 里点“卸载”".to_string(),
    ("java", "winget") => "卸载：winget uninstall --id EclipseAdoptium.Temurin.<主版本>.JDK -e；也可在 FoPanel 里点“卸载”".to_string(),
    ("java", "sdkman") => "卸载：sdk uninstall java <candidate>；也可在 FoPanel 里点“卸载”".to_string(),
    ("python", "framework") => "FoPanel 仅支持隐藏；卸载请通过系统/安装器处理".to_string(),
    (_, "standalone") => "FoPanel 仅支持隐藏；卸载请使用原安装方式处理".to_string(),
    (_, "path") => "FoPanel 仅支持隐藏；卸载请使用原安装方式处理".to_string(),
    _ => String::new(),
  };

  (upgrade, remove)
}

#[derive(Debug, Deserialize)]
pub struct ManualRuntimeInput {
  pub language: String,
  pub version: String,
  pub path: String,
}

#[tauri::command]
pub fn add_manual_runtime(app: AppHandle, runtime: ManualRuntimeInput) -> Result<(), String> {
  let path = PathBuf::from(&runtime.path);
  if !path.exists() {
    return Err("路径不存在".to_string());
  }

  let mut list = load_manual(&app)?;
  list.push(RuntimeVersion {
    language: runtime.language,
    version: runtime.version,
    path: runtime.path,
    active: false,
    source: "manual".to_string(),
  });

  save_manual(&app, &list)?;
  Ok(())
}

fn load_manual(app: &AppHandle) -> Result<Vec<RuntimeVersion>, String> {
  let file = manual_file_path(app)?;
  if !file.exists() {
    return Ok(vec![]);
  }
  let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
  let mut list: Vec<RuntimeVersion> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
  for item in list.iter_mut() {
    item.active = false;
    item.source = "manual".to_string();
  }
  Ok(list)
}

fn save_manual(app: &AppHandle, list: &[RuntimeVersion]) -> Result<(), String> {
  let file = manual_file_path(app)?;
  let json = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
  fs::write(file, json).map_err(|e| e.to_string())
}

fn manual_file_path(app: &AppHandle) -> Result<PathBuf, String> {
  let dir = app_data_dir(app)?;
  ensure_dir(&dir)?;
  Ok(dir.join("runtimes.manual.json"))
}

fn profiles_file_path(app: &AppHandle) -> Result<PathBuf, String> {
  let dir = app_data_dir(app)?;
  ensure_dir(&dir)?;
  Ok(dir.join("runtimes.profiles.json"))
}

fn installer_overrides_file_path(app: &AppHandle) -> Result<PathBuf, String> {
  let dir = app_data_dir(app)?;
  ensure_dir(&dir)?;
  Ok(dir.join("installers.overrides.json"))
}

fn load_installer_overrides(app: &AppHandle) -> Result<HashMap<String, String>, String> {
  let file = installer_overrides_file_path(app)?;
  if !file.exists() {
    return Ok(HashMap::new());
  }
  let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
  serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn save_installer_overrides(app: &AppHandle, map: &HashMap<String, String>) -> Result<(), String> {
  let file = installer_overrides_file_path(app)?;
  let json = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
  fs::write(file, json).map_err(|e| e.to_string())
}

fn default_profiles() -> Vec<RuntimeProfile> {
  vec![
    RuntimeProfile {
      id: "default-mac".to_string(),
      name: "默认开发环境（macOS）".to_string(),
      items: vec![
        RuntimeProfileItem {
          language: "node".to_string(),
          installer: "fnm".to_string(),
          version: "22.0.0".to_string(),
        },
        RuntimeProfileItem {
          language: "python".to_string(),
          installer: "pyenv".to_string(),
          version: "3.12.13".to_string(),
        },
        RuntimeProfileItem {
          language: "rust".to_string(),
          installer: "rustup".to_string(),
          version: "stable".to_string(),
        },
        RuntimeProfileItem {
          language: "bun".to_string(),
          installer: "homebrew".to_string(),
          version: "latest".to_string(),
        },
        RuntimeProfileItem {
          language: "deno".to_string(),
          installer: "homebrew".to_string(),
          version: "latest".to_string(),
        },
        RuntimeProfileItem {
          language: "go".to_string(),
          installer: "homebrew".to_string(),
          version: "latest".to_string(),
        },
        RuntimeProfileItem {
          language: "php".to_string(),
          installer: "homebrew".to_string(),
          version: "8.4".to_string(),
        },
        RuntimeProfileItem {
          language: "java".to_string(),
          installer: "homebrew".to_string(),
          version: "21".to_string(),
        },
      ],
    },
    RuntimeProfile {
      id: "default-mac-managers".to_string(),
      name: "默认开发环境（macOS·版本管理器示例）".to_string(),
      items: vec![
        RuntimeProfileItem {
          language: "node".to_string(),
          installer: "fnm".to_string(),
          version: "22.0.0".to_string(),
        },
        RuntimeProfileItem {
          language: "python".to_string(),
          installer: "pyenv".to_string(),
          version: "3.12.13".to_string(),
        },
        RuntimeProfileItem {
          language: "go".to_string(),
          installer: "goenv".to_string(),
          version: "1.22.0".to_string(),
        },
        RuntimeProfileItem {
          language: "php".to_string(),
          installer: "phpenv".to_string(),
          version: "8.3.0".to_string(),
        },
        RuntimeProfileItem {
          language: "java".to_string(),
          installer: "sdkman".to_string(),
          version: "21.0.2-tem".to_string(),
        },
      ],
    },
    RuntimeProfile {
      id: "default-win".to_string(),
      name: "默认开发环境（Windows）".to_string(),
      items: vec![
        RuntimeProfileItem {
          language: "node".to_string(),
          installer: "nvm".to_string(),
          version: "22.0.0".to_string(),
        },
        RuntimeProfileItem {
          language: "rust".to_string(),
          installer: "rustup".to_string(),
          version: "stable".to_string(),
        },
        RuntimeProfileItem {
          language: "bun".to_string(),
          installer: "winget".to_string(),
          version: "latest".to_string(),
        },
        RuntimeProfileItem {
          language: "deno".to_string(),
          installer: "winget".to_string(),
          version: "latest".to_string(),
        },
        RuntimeProfileItem {
          language: "go".to_string(),
          installer: "winget".to_string(),
          version: "latest".to_string(),
        },
        RuntimeProfileItem {
          language: "php".to_string(),
          installer: "winget".to_string(),
          version: "8.4".to_string(),
        },
        RuntimeProfileItem {
          language: "java".to_string(),
          installer: "winget".to_string(),
          version: "21".to_string(),
        },
      ],
    },
  ]
}

fn load_profiles(app: &AppHandle) -> Result<Vec<RuntimeProfile>, String> {
  let file = profiles_file_path(app)?;
  if !file.exists() {
    let list = default_profiles();
    save_profiles(app, &list)?;
    return Ok(list);
  }
  let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
  let mut list: Vec<RuntimeProfile> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
  if list.is_empty() {
    list = default_profiles();
    save_profiles(app, &list)?;
  }
  Ok(list)
}

fn save_profiles(app: &AppHandle, list: &[RuntimeProfile]) -> Result<(), String> {
  let file = profiles_file_path(app)?;
  let json = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
  fs::write(file, json).map_err(|e| e.to_string())
}

fn is_same_runtime(a: &RuntimeVersion, b: &ActivateRuntimeInput) -> bool {
  a.language == b.language && a.version == b.version && a.path == b.path && a.source == b.source
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveRuntime {
  version: String,
  path: String,
  source: String,
}

fn is_match_active(item: &RuntimeVersion, sel: &ActiveRuntime) -> bool {
  if item.version != sel.version {
    return false;
  }
  if item.source != sel.source {
    return false;
  }
  if item.path.is_empty() || sel.path.is_empty() {
    return true;
  }
  item.path == sel.path
}

fn cleanup_active_if_needed(app: &AppHandle, runtime: &ActivateRuntimeInput) -> Result<(), String> {
  let mut active = load_active(app)?;
  let Some(sel) = active.get(&runtime.language) else {
    return Ok(());
  };
  let temp = RuntimeVersion {
    language: runtime.language.clone(),
    version: runtime.version.clone(),
    path: runtime.path.clone(),
    active: false,
    source: runtime.source.clone(),
  };
  if is_match_active(&temp, sel) {
    active.remove(&runtime.language);
    save_active(app, &active)?;
  }
  Ok(())
}

fn load_active(app: &AppHandle) -> Result<HashMap<String, ActiveRuntime>, String> {
  let file = active_file_path(app)?;
  if !file.exists() {
    return Ok(HashMap::new());
  }
  let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
  serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn save_active(app: &AppHandle, active: &HashMap<String, ActiveRuntime>) -> Result<(), String> {
  let file = active_file_path(app)?;
  let json = serde_json::to_string_pretty(active).map_err(|e| e.to_string())?;
  fs::write(file, json).map_err(|e| e.to_string())
}

fn active_file_path(app: &AppHandle) -> Result<PathBuf, String> {
  let dir = app_data_dir(app)?;
  ensure_dir(&dir)?;
  Ok(dir.join("runtimes.active.json"))
}

fn shims_dir(app: &AppHandle) -> Result<PathBuf, String> {
  let dir = app_data_dir(app)?;
  Ok(dir.join("shims"))
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
  app.path().app_data_dir().map_err(|e| e.to_string())
}

fn installer_bootstrap_hint(installer: &str, brew: bool, winget: bool) -> String {
  match installer {
    "homebrew" => {
      if cfg!(windows) {
        return "Homebrew 仅适用于 macOS/Linux".to_string();
      }
      if brew {
        return "brew update".to_string();
      }
      "/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"".to_string()
    }
    "winget" => {
      if cfg!(windows) {
        if winget {
          return "winget --version".to_string();
        }
        return "请安装 Microsoft Store 的 App Installer（包含 winget）".to_string();
      }
      "winget 仅适用于 Windows".to_string()
    }
    "fnm" => {
      if cfg!(windows) {
        if winget {
          return "winget install Schniz.fnm".to_string();
        }
        return "请先安装 winget 或使用 fnm 官方安装说明".to_string();
      }
      if brew {
        return "brew install fnm".to_string();
      }
      "curl -fsSL https://fnm.vercel.app/install | bash".to_string()
    }
    "nvm" => {
      if cfg!(windows) {
        if winget {
          return "winget install CoreyButler.NVMforWindows\n若已安装但未识别：请把 nvm.exe 所在目录加入 PATH，或设置 NVM_HOME 环境变量".to_string();
        }
        return "请先安装 winget 或手动安装 nvm-windows\n若已安装但未识别：请把 nvm.exe 所在目录加入 PATH，或设置 NVM_HOME 环境变量".to_string();
      }
      if brew {
        return "brew install nvm".to_string();
      }
      "curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash".to_string()
    }
    "pyenv" => {
      if cfg!(windows) {
        return "Windows 建议使用 pyenv-win 或直接安装 Python（后续可在 FoPanel 中手动添加）".to_string();
      }
      if brew {
        return "brew install pyenv".to_string();
      }
      "建议安装 Homebrew 后执行：brew install pyenv".to_string()
    }
    "rustup" => {
      if cfg!(windows) {
        if winget {
          return "winget install Rustlang.Rustup".to_string();
        }
        return "请先安装 winget 或访问 https://rustup.rs 安装 rustup".to_string();
      }
      "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y".to_string()
    }
    "goenv" => {
      if cfg!(windows) {
        return "Windows 暂不建议 goenv：可使用 winget 安装 Go（或自行配置多版本方案）".to_string();
      }
      if brew {
        return "brew install goenv".to_string();
      }
      "建议安装 Homebrew 后执行：brew install goenv".to_string()
    }
    "phpenv" => {
      if cfg!(windows) {
        return "Windows 暂不建议 phpenv：可使用 winget 安装 PHP（或自行配置多版本方案）".to_string();
      }
      if brew {
        return "可选：brew install phpenv（如不可用请按官方文档安装）".to_string();
      }
      "建议：安装 phpenv 后执行 phpenv init，并确保 $HOME/.phpenv/shims 在 PATH 前面".to_string()
    }
    "sdkman" => {
      if cfg!(windows) {
        return "Windows 建议使用 winget 安装 Java；SDKMAN 更适合 macOS/Linux（或在 WSL 中使用）".to_string();
      }
      "curl -s \"https://get.sdkman.io\" | bash && source \"$HOME/.sdkman/bin/sdkman-init.sh\"".to_string()
    }
    _ => "未知安装器".to_string(),
  }
}

fn installer_install_command(installer: &str, brew: bool, winget: bool) -> Option<String> {
  match installer {
    "homebrew" => None,
    "winget" => None,
    "fnm" => {
      if cfg!(windows) {
        if winget {
          return Some("winget install Schniz.fnm".to_string());
        }
        return None;
      }
      if brew {
        return Some("brew install fnm".to_string());
      }
      Some("curl -fsSL https://fnm.vercel.app/install | bash".to_string())
    }
    "nvm" => {
      if cfg!(windows) {
        if winget {
          return Some("winget install CoreyButler.NVMforWindows".to_string());
        }
        return None;
      }
      if brew {
        return Some("brew install nvm".to_string());
      }
      Some("curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash".to_string())
    }
    "pyenv" => {
      if cfg!(windows) {
        return None;
      }
      if brew {
        return Some("brew install pyenv".to_string());
      }
      None
    }
    "rustup" => {
      if cfg!(windows) {
        if winget {
          return Some("winget install Rustlang.Rustup".to_string());
        }
        return None;
      }
      Some("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y".to_string())
    }
    "goenv" => {
      if cfg!(windows) {
        return None;
      }
      if brew {
        return Some("brew install goenv".to_string());
      }
      None
    }
    "phpenv" => {
      if cfg!(windows) {
        return None;
      }
      if brew {
        return Some("brew install phpenv".to_string());
      }
      None
    }
    "sdkman" => {
      if cfg!(windows) {
        return None;
      }
      Some("curl -s \"https://get.sdkman.io\" | bash".to_string())
    }
    _ => None,
  }
}

fn installer_env_config(installer: &str) -> String {
  match installer {
    "homebrew" => {
      if cfg!(windows) {
        return "Homebrew 仅适用于 macOS/Linux。".to_string();
      }
      "安装后请重新打开终端；如果 brew 命令仍不可用，请把 Homebrew 的 shellenv 输出加入你的 shell 配置文件（例如 ~/.zshrc）。常用命令：\n(eval \"$(/opt/homebrew/bin/brew shellenv)\")".to_string()
    }
    "winget" => {
      if cfg!(windows) {
        return "winget 属于 Windows 的 App Installer。安装后建议重启 FoPanel；如果仍不可用，请检查系统 PATH 或更新 App Installer。".to_string();
      }
      "winget 仅适用于 Windows。".to_string()
    }
    "fnm" => {
      if cfg!(windows) {
        "安装后请重启 FoPanel；如果未识别，请把 fnm.exe 所在目录加入 PATH。".to_string()
      } else {
        "安装后请重新打开终端；若需要在 shell 中自动启用 fnm，可将 fnm env 相关初始化写入 shell 配置文件（按 fnm 官方文档）。".to_string()
      }
    }
    "nvm" => {
      if cfg!(windows) {
        "安装后请重启 FoPanel；若未识别：\n- 设置 NVM_HOME 指向 nvm 安装目录（目录内应有 nvm.exe）\n- 设置 NVM_SYMLINK 指向 nodejs 软链目录（常见：C:\\Program Files\\nodejs）\n- 确保 nvm.exe 所在目录在 PATH 中".to_string()
      } else {
        "安装后请重新打开终端；macOS/Linux 需要在 shell 配置中加载 nvm（例如 ~/.zshrc）并确保 NVM_DIR 正确。".to_string()
      }
    }
    "pyenv" => {
      if cfg!(windows) {
        "Windows 建议使用 pyenv-win 或直接安装 Python。".to_string()
      } else {
        "安装后请重新打开终端；并把 pyenv init 输出加入你的 shell 配置文件（按 pyenv 官方文档），确保 pyenv/shims 在 PATH 前面。".to_string()
      }
    }
    "rustup" => {
      if cfg!(windows) {
        "安装后请重启 FoPanel；并确认 rustup.exe 在 PATH 中。".to_string()
      } else {
        "安装后请重新打开终端；并确保 ~/.cargo/bin 在 PATH 中（rustup 安装器一般会自动写入）。".to_string()
      }
    }
    "goenv" => {
      if cfg!(windows) {
        "Windows 暂不建议 goenv，可优先使用 winget 安装 Go。".to_string()
      } else {
        "安装后请重新打开终端；并把 goenv init 输出加入 shell 配置文件，确保 goenv/shims 在 PATH 前面。".to_string()
      }
    }
    "phpenv" => {
      if cfg!(windows) {
        "Windows 暂不建议 phpenv，可优先使用 winget 安装 PHP。".to_string()
      } else {
        "安装后请重新打开终端；并执行 phpenv init（按官方文档），确保 $HOME/.phpenv/shims 在 PATH 前面。".to_string()
      }
    }
    "sdkman" => {
      if cfg!(windows) {
        "Windows 建议使用 winget 安装 Java；SDKMAN 更适合 macOS/Linux（或在 WSL 中使用）。".to_string()
      } else {
        "安装后请重新打开终端；并执行：source \"$HOME/.sdkman/bin/sdkman-init.sh\"。如需长期生效，把 source 行写入 ~/.zshrc 或 ~/.bashrc。".to_string()
      }
    }
    _ => "未知安装器。".to_string(),
  }
}

fn resolve_exec_path(runtime: &ActivateRuntimeInput) -> Result<PathBuf, String> {
  if !runtime.path.trim().is_empty() {
    return Ok(PathBuf::from(runtime.path.trim()));
  }
  if runtime.source == "fnm" && runtime.language == "node" {
    return resolve_node_from_fnm(&runtime.version);
  }
  Err("缺少可执行文件路径".to_string())
}

fn resolve_node_from_fnm(version: &str) -> Result<PathBuf, String> {
  let v = if version.starts_with('v') {
    version.to_string()
  } else {
    format!("v{}", version)
  };
  let out = Command::new("fnm")
    .args(["exec", "--using", &v, "node", "-p", "process.execPath"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&out);
  let s = text.trim().to_string();
  if s.is_empty() {
    return Err("无法通过 fnm 解析 node 路径".to_string());
  }
  Ok(PathBuf::from(s))
}

fn shell_escape(s: &str) -> String {
  let mut out = String::new();
  out.push('\'');
  for ch in s.chars() {
    if ch == '\'' {
      out.push_str("'\\''");
    } else {
      out.push(ch);
    }
  }
  out.push('\'');
  out
}

fn activate_node(shims: &Path, node_exec: &Path) -> Result<(), String> {
  let bin = node_exec
    .parent()
    .ok_or_else(|| "node 路径无效".to_string())?;
  let pairs = [
    ("node", node_exec.to_path_buf()),
    ("npm", bin.join("npm")),
    ("npx", bin.join("npx")),
    ("corepack", bin.join("corepack")),
  ];
  for (name, target) in pairs {
    if !target.exists() {
      continue;
    }
    link_exec(shims, name, &target)?;
  }
  Ok(())
}

fn activate_bun(shims: &Path, bun_exec: &Path) -> Result<(), String> {
  let bin = bun_exec
    .parent()
    .ok_or_else(|| "bun 路径无效".to_string())?;
  let pairs = [("bun", bun_exec.to_path_buf()), ("bunx", bin.join("bunx"))];
  for (name, target) in pairs {
    if !target.exists() {
      continue;
    }
    link_exec(shims, name, &target)?;
  }
  Ok(())
}

fn activate_python(shims: &Path, py_exec: &Path) -> Result<(), String> {
  let bin = py_exec
    .parent()
    .ok_or_else(|| "python 路径无效".to_string())?;
  let python_name = py_exec
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("python3");
  let mut pairs = Vec::<(&str, PathBuf)>::new();
  if python_name == "python" {
    pairs.push(("python", py_exec.to_path_buf()));
    let python3 = bin.join("python3");
    if python3.exists() {
      pairs.push(("python3", python3));
    }
  } else {
    pairs.push(("python3", py_exec.to_path_buf()));
    let python = bin.join("python");
    if python.exists() {
      pairs.push(("python", python));
    }
  }

  let pip = bin.join("pip");
  if pip.exists() {
    pairs.push(("pip", pip));
  }
  let pip3 = bin.join("pip3");
  if pip3.exists() {
    pairs.push(("pip3", pip3));
  }

  for (name, target) in pairs {
    link_exec(shims, name, &target)?;
  }
  Ok(())
}

fn activate_simple(shims: &Path, name: &str, exec: &Path) -> Result<(), String> {
  link_exec(shims, name, exec)?;
  Ok(())
}

fn activate_rust(shims: &Path, rustc_exec: &Path) -> Result<(), String> {
  let bin = rustc_exec
    .parent()
    .ok_or_else(|| "rustc 路径无效".to_string())?;
  let pairs = [
    ("rustc", rustc_exec.to_path_buf()),
    ("cargo", bin.join("cargo")),
    ("rustup", bin.join("rustup")),
  ];
  for (name, target) in pairs {
    if !target.exists() {
      continue;
    }
    link_exec(shims, name, &target)?;
  }
  Ok(())
}

fn get_python_detail(exec: &Path) -> Result<(Vec<RuntimePackage>, HashMap<String, String>), String> {
  let mut info = HashMap::<String, String>::new();

  let pip_ver = Command::new(exec)
    .args(["-m", "pip", "--version"])
    .output()
    .ok()
    .map(|o| merge_output(&o).0.trim().to_string())
    .unwrap_or_default();
  if !pip_ver.is_empty() {
    info.insert("pip".to_string(), pip_ver);
  }

  let output = Command::new(exec)
    .args(["-m", "pip", "list", "--format=json"])
    .output()
    .map_err(|e| e.to_string())?;
  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let text = stdout.trim();
  if text.is_empty() {
    return Ok((vec![], info));
  }

  #[derive(Deserialize)]
  struct PipItem {
    name: String,
    version: String,
  }
  let json = extract_json_array(text).unwrap_or_else(|| text.to_string());
  let items: Vec<PipItem> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
  let mut packages = items
    .into_iter()
    .map(|i| RuntimePackage {
      name: i.name,
      version: i.version,
    })
    .collect::<Vec<_>>();
  packages.sort_by(|a, b| a.name.cmp(&b.name));

  Ok((packages, info))
}

fn get_node_detail(exec: &Path) -> Result<(Vec<RuntimePackage>, HashMap<String, String>), String> {
  let mut info = HashMap::<String, String>::new();
  let bin = exec
    .parent()
    .ok_or_else(|| "node 路径无效".to_string())?;
  let npm = if cfg!(windows) {
    let c = bin.join("npm.cmd");
    if c.exists() {
      c
    } else {
      bin.join("npm")
    }
  } else {
    bin.join("npm")
  };
  if !npm.exists() {
    return Ok((vec![], info));
  }

  let npm_ver = if cfg!(windows) {
    Command::new("cmd")
      .args(["/C", &npm.to_string_lossy(), "-v"])
      .output()
      .ok()
      .map(|o| merge_output(&o).0.trim().to_string())
      .unwrap_or_default()
  } else {
    Command::new(&npm)
      .args(["-v"])
      .output()
      .ok()
      .map(|o| merge_output(&o).0.trim().to_string())
      .unwrap_or_default()
  };
  if !npm_ver.is_empty() {
    info.insert("npm".to_string(), npm_ver);
  }

  let output = if cfg!(windows) {
    Command::new("cmd")
      .args([
        "/C",
        &npm.to_string_lossy(),
        "ls",
        "-g",
        "--depth=0",
        "--json",
      ])
      .output()
      .map_err(|e| e.to_string())?
  } else {
    Command::new(&npm)
      .args(["ls", "-g", "--depth=0", "--json"])
      .output()
      .map_err(|e| e.to_string())?
  };
  let (text, _) = merge_output(&output);
  let text = text.trim();
  if text.is_empty() {
    return Ok((vec![], info));
  }

  #[derive(Deserialize)]
  struct NpmDep {
    version: Option<String>,
  }
  #[derive(Deserialize)]
  struct NpmTree {
    dependencies: Option<HashMap<String, NpmDep>>,
  }
  let tree: NpmTree = serde_json::from_str(text).map_err(|e| e.to_string())?;
  let mut packages = Vec::<RuntimePackage>::new();
  if let Some(deps) = tree.dependencies {
    for (name, dep) in deps {
      packages.push(RuntimePackage {
        name,
        version: dep.version.unwrap_or_default(),
      });
    }
  }
  packages.sort_by(|a, b| a.name.cmp(&b.name));

  Ok((packages, info))
}

fn get_rust_detail(exec: &Path) -> Result<(Vec<RuntimePackage>, HashMap<String, String>), String> {
  let mut info = HashMap::<String, String>::new();
  let bin = exec
    .parent()
    .ok_or_else(|| "rustc 路径无效".to_string())?;
  let cargo = bin.join("cargo");
  let rustc_ver = Command::new(exec)
    .args(["--version"])
    .output()
    .ok()
    .map(|o| merge_output(&o).0.trim().to_string())
    .unwrap_or_default();
  if !rustc_ver.is_empty() {
    info.insert("rustc".to_string(), rustc_ver);
  }
  if cargo.exists() {
    let cargo_ver = Command::new(&cargo)
      .args(["--version"])
      .output()
      .ok()
      .map(|o| merge_output(&o).0.trim().to_string())
      .unwrap_or_default();
    if !cargo_ver.is_empty() {
      info.insert("cargo".to_string(), cargo_ver);
    }

    let output = Command::new(&cargo)
      .args(["install", "--list"])
      .output()
      .map_err(|e| e.to_string())?;
    let (text, _) = merge_output(&output);
    let mut packages = Vec::<RuntimePackage>::new();
    for line in text.lines() {
      let l = line.trim();
      if !l.ends_with(':') {
        continue;
      }
      let l = l.trim_end_matches(':');
      let mut parts = l.split_whitespace();
      let name = parts.next().unwrap_or_default().to_string();
      let version = parts
        .next()
        .and_then(|v| v.strip_prefix('v'))
        .unwrap_or_default()
        .to_string();
      if !name.is_empty() {
        packages.push(RuntimePackage { name, version });
      }
    }
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    return Ok((packages, info));
  }

  Ok((vec![], info))
}

fn get_go_detail(exec: &Path) -> Result<(Vec<RuntimePackage>, HashMap<String, String>), String> {
  let mut info = HashMap::<String, String>::new();
  let output = Command::new(exec)
    .args(["env", "-json"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  if let Ok(map) = serde_json::from_str::<HashMap<String, serde_json::Value>>(text.trim()) {
    for key in ["GOROOT", "GOPATH", "GOMODCACHE", "GOOS", "GOARCH"] {
      if let Some(v) = map.get(key) {
        if let Some(s) = v.as_str() {
          info.insert(key.to_string(), s.to_string());
        }
      }
    }
  }
  Ok((vec![], info))
}

fn get_php_detail(exec: &Path) -> Result<(Vec<RuntimePackage>, HashMap<String, String>), String> {
  let mut info = HashMap::<String, String>::new();
  let php_ver = Command::new(exec)
    .args(["-v"])
    .output()
    .ok()
    .map(|o| merge_output(&o).0.lines().next().unwrap_or_default().trim().to_string())
    .unwrap_or_default();
  if !php_ver.is_empty() {
    info.insert("php".to_string(), php_ver);
  }

  let output = Command::new(exec)
    .args(["-m"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  let mut packages = Vec::<RuntimePackage>::new();
  for line in text.lines() {
    let l = line.trim();
    if l.is_empty() {
      continue;
    }
    if l.starts_with('[') {
      continue;
    }
    packages.push(RuntimePackage {
      name: l.to_string(),
      version: String::new(),
    });
  }
  packages.sort_by(|a, b| a.name.cmp(&b.name));
  Ok((packages, info))
}

fn get_java_detail(exec: &Path) -> Result<(Vec<RuntimePackage>, HashMap<String, String>), String> {
  let mut info = HashMap::<String, String>::new();
  let output = Command::new(exec)
    .args(["-version"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  let java_ver = text.trim().to_string();
  if !java_ver.is_empty() {
    info.insert("java".to_string(), java_ver);
  }

  if let Some(bin) = exec.parent() {
    if let Some(home) = bin.parent() {
      info.insert("JAVA_HOME(推断)".to_string(), home.to_string_lossy().to_string());
    }
    let javac = bin.join(if cfg!(windows) { "javac.exe" } else { "javac" });
    if javac.exists() {
      let out = Command::new(&javac)
        .args(["-version"])
        .output()
        .ok()
        .map(|o| merge_output(&o).0.trim().to_string())
        .unwrap_or_default();
      if !out.is_empty() {
        info.insert("javac".to_string(), out);
      }
    }
  }

  Ok((vec![], info))
}

fn link_exec(shims: &Path, name: &str, target: &Path) -> Result<(), String> {
  let link = shims.join(name);
  if link.exists() {
    fs::remove_file(&link).map_err(|e| e.to_string())?;
  }
  #[cfg(unix)]
  {
    std::os::unix::fs::symlink(target, link).map_err(|e| e.to_string())?;
    return Ok(());
  }
  #[cfg(windows)]
  {
    std::os::windows::fs::symlink_file(target, link).map_err(|e| e.to_string())?;
    return Ok(());
  }
  #[allow(unreachable_code)]
  Err("当前平台不支持软链接".to_string())
}

fn verify_node(shims: &Path, expected: &str) -> Result<ActivateRuntimeResult, String> {
  let exec = shims.join("node");
  if !exec.exists() {
    return Err("node 未生成到 shims".to_string());
  }
  let output = Command::new(exec)
    .args(["-v"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  let actual = text.trim().strip_prefix('v').unwrap_or(text.trim()).to_string();
  let ok = actual == expected || actual.starts_with(expected);
  Ok(ActivateRuntimeResult {
    verified: ok,
    output: text.trim().to_string(),
    expected: expected.to_string(),
    actual,
  })
}

fn verify_python(shims: &Path, expected: &str) -> Result<ActivateRuntimeResult, String> {
  let exec = if shims.join("python3").exists() {
    shims.join("python3")
  } else {
    shims.join("python")
  };
  if !exec.exists() {
    return Err("python 未生成到 shims".to_string());
  }
  let output = Command::new(exec)
    .args(["--version"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, prefer_stderr) = merge_output(&output);
  let raw = if prefer_stderr {
    String::from_utf8_lossy(&output.stderr).to_string()
  } else {
    text.clone()
  };
  let line = raw.trim();
  let actual = line
    .strip_prefix("Python ")
    .unwrap_or(line)
    .trim()
    .to_string();
  let ok = actual == expected || actual.starts_with(expected);
  Ok(ActivateRuntimeResult {
    verified: ok,
    output: line.to_string(),
    expected: expected.to_string(),
    actual,
  })
}

fn verify_bun(shims: &Path, expected: &str) -> Result<ActivateRuntimeResult, String> {
  let exec = shims.join("bun");
  if !exec.exists() {
    return Err("bun 未生成到 shims".to_string());
  }
  let output = Command::new(exec)
    .args(["--version"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  let line = text.trim();
  let actual = extract_semver_like(line).unwrap_or_else(|| line.to_string());
  let ok = actual == expected || actual.starts_with(expected);
  Ok(ActivateRuntimeResult {
    verified: ok,
    output: line.to_string(),
    expected: expected.to_string(),
    actual,
  })
}

fn verify_deno(shims: &Path, expected: &str) -> Result<ActivateRuntimeResult, String> {
  let exec = shims.join("deno");
  if !exec.exists() {
    return Err("deno 未生成到 shims".to_string());
  }
  let output = Command::new(exec)
    .args(["--version"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  let mut actual = String::new();
  for line in text.lines() {
    let l = line.trim();
    if l.starts_with("deno ") {
      actual = extract_semver_like(l).unwrap_or_default();
      break;
    }
  }
  if actual.is_empty() {
    actual = extract_semver_like(text.trim()).unwrap_or_default();
  }
  let ok = actual == expected || actual.starts_with(expected);
  Ok(ActivateRuntimeResult {
    verified: ok,
    output: text.trim().to_string(),
    expected: expected.to_string(),
    actual,
  })
}

fn verify_go(shims: &Path, expected: &str) -> Result<ActivateRuntimeResult, String> {
  let exec = shims.join("go");
  if !exec.exists() {
    return Err("go 未生成到 shims".to_string());
  }
  let output = Command::new(exec)
    .args(["version"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  let line = text.trim();
  let actual = line
    .split_whitespace()
    .find(|x| x.starts_with("go"))
    .map(|x| x.trim_start_matches("go").to_string())
    .unwrap_or_default();
  let ok = actual == expected || actual.starts_with(expected);
  Ok(ActivateRuntimeResult {
    verified: ok,
    output: line.to_string(),
    expected: expected.to_string(),
    actual,
  })
}

fn verify_php(shims: &Path, expected: &str) -> Result<ActivateRuntimeResult, String> {
  let exec = shims.join("php");
  if !exec.exists() {
    return Err("php 未生成到 shims".to_string());
  }
  let output = Command::new(exec)
    .args(["-v"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  let line = text.lines().next().unwrap_or_default().trim();
  let actual = extract_semver_like(line).unwrap_or_default();
  let ok = actual == expected || actual.starts_with(expected);
  Ok(ActivateRuntimeResult {
    verified: ok,
    output: line.to_string(),
    expected: expected.to_string(),
    actual,
  })
}

fn verify_rust(shims: &Path, expected: &str) -> Result<ActivateRuntimeResult, String> {
  let exec = shims.join("rustc");
  if !exec.exists() {
    return Err("rustc 未生成到 shims".to_string());
  }
  let output = Command::new(exec)
    .args(["--version"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&output);
  let line = text.trim();
  let actual = extract_semver_like(line).unwrap_or_default();
  let ok = actual == expected || actual.starts_with(expected);
  Ok(ActivateRuntimeResult {
    verified: ok,
    output: line.to_string(),
    expected: expected.to_string(),
    actual,
  })
}

fn merge_output(output: &std::process::Output) -> (String, bool) {
  let out = String::from_utf8_lossy(&output.stdout).to_string();
  let err = String::from_utf8_lossy(&output.stderr).to_string();
  if !err.trim().is_empty() && out.trim().is_empty() {
    return (err, true);
  }
  if out.trim().is_empty() && err.trim().is_empty() {
    return (String::new(), false);
  }
  if err.trim().is_empty() {
    return (out, false);
  }
  (format!("{out}\n{err}"), false)
}

fn extract_json_array(s: &str) -> Option<String> {
  let start = s.find('[')?;
  let end = s.rfind(']')?;
  if end < start {
    return None;
  }
  Some(s[start..=end].to_string())
}

fn extract_semver_like(s: &str) -> Option<String> {
  let bytes = s.as_bytes();
  for i in 0..bytes.len() {
    let c = bytes[i] as char;
    if !(c.is_ascii_digit() || c == 'v') {
      continue;
    }
    let mut j = i;
    while j < bytes.len() {
      let c2 = bytes[j] as char;
      if c2.is_ascii_alphanumeric() || c2 == '.' || c2 == '-' {
        j += 1;
      } else {
        break;
      }
    }
    let token = s[i..j].trim();
    let token = token.strip_prefix('v').unwrap_or(token);
    if token.chars().any(|x| x == '.') {
      return Some(token.to_string());
    }
  }
  None
}

fn has_command(name: &str) -> bool {
  find_in_path(name).is_some()
}

fn is_executable(path: &Path) -> bool {
  if !path.is_file() {
    return false;
  }
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
      return meta.permissions().mode() & 0o111 != 0;
    }
  }
  #[cfg(windows)]
  {
    let ext = path
      .extension()
      .and_then(|x| x.to_str())
      .unwrap_or("")
      .to_ascii_lowercase();
    if ext.is_empty() {
      return false;
    }
    let exts = env::var_os("PATHEXT")
      .map(|x| x.to_string_lossy().to_string())
      .unwrap_or_else(|| ".EXE;.COM".to_string());
    return exts
      .split(';')
      .map(|x| x.trim().trim_start_matches('.').to_ascii_lowercase())
      .any(|x| x == ext);
  }
  #[allow(unreachable_code)]
  false
}

fn find_in_path(program: &str) -> Option<PathBuf> {
  let path = env::var_os("PATH")?;
  #[cfg(windows)]
  {
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
        if is_executable(&candidate) {
          return Some(candidate);
        }
        let candidate = PathBuf::from(format!("{}{}", raw.to_string_lossy(), ext.to_lowercase()));
        if is_executable(&candidate) {
          return Some(candidate);
        }
      }
    }
    return None;
  }
  #[cfg(not(windows))]
  {
    for dir in env::split_paths(&path) {
      let candidate = dir.join(program);
      if is_executable(&candidate) {
        return Some(candidate);
      }
    }
    None
  }
}

fn nvm_sh_exists() -> bool {
  if let Some(dir) = env::var_os("NVM_DIR") {
    let p = PathBuf::from(dir).join("nvm.sh");
    if p.exists() {
      return true;
    }
  }
  if let Ok(home) = env::var("HOME") {
    let p = PathBuf::from(home).join(".nvm").join("nvm.sh");
    return p.exists();
  }
  false
}

fn nvm_exe_path(overrides: &HashMap<String, String>) -> Option<PathBuf> {
  if let Some(p) = overrides.get("nvm") {
    let p = PathBuf::from(p);
    if p.exists() {
      return Some(p);
    }
  }
  if let Some(p) = find_in_path("nvm") {
    return Some(p);
  }
  if let Some(home) = env::var_os("NVM_HOME") {
    let p = PathBuf::from(home).join("nvm.exe");
    if p.exists() {
      return Some(p);
    }
  }
  if let Some(appdata) = env::var_os("APPDATA") {
    let p = PathBuf::from(appdata).join("nvm").join("nvm.exe");
    if p.exists() {
      return Some(p);
    }
  }
  if let Some(local) = env::var_os("LOCALAPPDATA") {
    let p = PathBuf::from(local).join("nvm").join("nvm.exe");
    if p.exists() {
      return Some(p);
    }
  }
  let p = PathBuf::from(r"C:\Program Files\nvm\nvm.exe");
  if p.exists() {
    return Some(p);
  }
  let p = PathBuf::from(r"C:\Program Files (x86)\nvm\nvm.exe");
  if p.exists() {
    return Some(p);
  }
  None
}

fn nvm_program(overrides: &HashMap<String, String>) -> Result<String, String> {
  nvm_exe_path(overrides)
    .map(|p| p.to_string_lossy().to_string())
    .ok_or_else(|| "未找到 nvm（nvm-windows）。如果你已安装但 FoPanel 未识别：请把 nvm.exe 所在目录加入 PATH，或设置 NVM_HOME 环境变量指向 nvm 安装目录。".to_string())
}

fn run_install_fnm(version: &str) -> Result<String, String> {
  let out = Command::new("fnm")
    .args(["install", version])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_install_nvm(version: &str, overrides: &HashMap<String, String>) -> Result<String, String> {
  if cfg!(windows) {
    let out = Command::new(nvm_program(overrides)?)
      .args(["install", version])
      .output()
      .map_err(|e| e.to_string())?;
    return Ok(merge_output(&out).0.trim().to_string());
  }
  let script = format!(
    "export NVM_DIR=\"${{NVM_DIR:-$HOME/.nvm}}\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"; nvm install {v}; nvm list",
    v = shell_escape(version)
  );
  let out = Command::new("zsh")
    .args(["-lc", &script])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_install_pyenv(version: &str) -> Result<String, String> {
  let out = Command::new("pyenv")
    .args(["install", "-s", version])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_install_rustup(version: &str) -> Result<String, String> {
  let out = Command::new("rustup")
    .args(["toolchain", "install", version])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_install_homebrew(formula: &str, _version: &str) -> Result<String, String> {
  if cfg!(windows) {
    return Err("当前系统不支持 Homebrew 安装器".to_string());
  }
  let out = Command::new("brew")
    .args(["install", formula])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_install_winget(id: &str, version: &str) -> Result<String, String> {
  if !cfg!(windows) {
    return Err("当前系统不支持 winget 安装器".to_string());
  }
  let mut args = vec![
    "install".to_string(),
    "--id".to_string(),
    id.to_string(),
    "-e".to_string(),
    "--accept-source-agreements".to_string(),
    "--accept-package-agreements".to_string(),
  ];
  let v = version.trim();
  if !v.is_empty() && v != "latest" && v != "stable" {
    args.push("--version".to_string());
    args.push(v.to_string());
  }
  let out = Command::new("winget")
    .args(args)
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_install_goenv(version: &str) -> Result<String, String> {
  let out = Command::new("goenv")
    .args(["install", "-s", version])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_uninstall_goenv(version: &str) -> Result<String, String> {
  if !goenv_has_uninstall()? {
    return Err("当前 goenv 未提供 uninstall 命令，请安装 goenv-uninstall 或手动删除版本目录".to_string());
  }
  let out = Command::new("goenv")
    .args(["uninstall", "-f", version])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn goenv_has_uninstall() -> Result<bool, String> {
  let out = Command::new("goenv")
    .args(["commands"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&out);
  Ok(text.lines().any(|l| l.trim() == "uninstall"))
}

fn run_install_phpenv(version: &str, overrides: &HashMap<String, String>) -> Result<String, String> {
  let v = version.trim();
  if v.is_empty() || v == "latest" || v == "stable" {
    return Err("phpenv 需要明确版本号（例如 8.4.0）".to_string());
  }
  let program = overrides.get("phpenv").map(|x| x.as_str()).unwrap_or("phpenv");
  let out = Command::new(program)
    .args(["install", v])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_uninstall_phpenv(
  version: &str,
  overrides: &HashMap<String, String>,
) -> Result<String, String> {
  if !phpenv_has_uninstall(overrides)? {
    return Err("当前 phpenv 未提供 uninstall 命令，请安装 phpenv-uninstall 或手动删除版本目录".to_string());
  }
  let program = overrides.get("phpenv").map(|x| x.as_str()).unwrap_or("phpenv");
  let out = Command::new(program)
    .args(["uninstall", "-f", version])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn phpenv_has_uninstall(overrides: &HashMap<String, String>) -> Result<bool, String> {
  let program = overrides.get("phpenv").map(|x| x.as_str()).unwrap_or("phpenv");
  let out = Command::new(program)
    .args(["commands"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&out);
  Ok(text.lines().any(|l| l.trim() == "uninstall"))
}

fn sdkman_is_available() -> bool {
  if cfg!(windows) {
    return false;
  }
  let script =
    "export SDKMAN_DIR=\"${SDKMAN_DIR:-$HOME/.sdkman}\"; [ -s \"$SDKMAN_DIR/bin/sdkman-init.sh\" ] || exit 1; source \"$SDKMAN_DIR/bin/sdkman-init.sh\"; type -t sdk 2>/dev/null";
  let out = Command::new("bash").args(["-lc", script]).output();
  let Ok(out) = out else {
    return false;
  };
  let (text, _) = merge_output(&out);
  let t = text.trim();
  t == "function" || t == "file"
}

fn sdkman_source_line(overrides: &HashMap<String, String>) -> String {
  if let Some(p) = overrides.get("sdkman") {
    let p = PathBuf::from(p);
    if p.exists() {
      if let Some(dir) = p.parent().and_then(|x| x.parent()) {
        return format!(
          "export SDKMAN_DIR={d}; source {s}",
          d = shell_escape(dir.to_string_lossy().as_ref()),
          s = shell_escape(p.to_string_lossy().as_ref())
        );
      }
      return format!("source {}", shell_escape(p.to_string_lossy().as_ref()));
    }
  }
  "export SDKMAN_DIR=\"${SDKMAN_DIR:-$HOME/.sdkman}\"; source \"$SDKMAN_DIR/bin/sdkman-init.sh\"".to_string()
}

fn run_install_sdkman_java(
  candidate: &str,
  overrides: &HashMap<String, String>,
) -> Result<String, String> {
  if cfg!(windows) {
    return Err("Windows 不支持直接使用 SDKMAN（建议使用 winget，或在 WSL 中安装 SDKMAN）".to_string());
  }
  let c = candidate.trim();
  if c.is_empty() || c == "latest" || c == "stable" {
    return Err("SDKMAN 需要 Java candidate（例如 21.0.2-tem / 17.0.10-zulu）".to_string());
  }
  let source = sdkman_source_line(overrides);
  let script = format!(
    "{source}; export SDKMAN_NON_INTERACTIVE=true; sdk install java {c}",
    source = source,
    c = shell_escape(c)
  );
  let out = Command::new("bash")
    .args(["-lc", &script])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_uninstall_sdkman_java(
  candidate: &str,
  overrides: &HashMap<String, String>,
) -> Result<String, String> {
  if cfg!(windows) {
    return Err("Windows 不支持直接使用 SDKMAN（建议使用 winget，或在 WSL 中安装 SDKMAN）".to_string());
  }
  let c = candidate.trim();
  if c.is_empty() || c == "latest" || c == "stable" {
    return Err("SDKMAN 需要 Java candidate（例如 21.0.2-tem / 17.0.10-zulu）".to_string());
  }
  let source = sdkman_source_line(overrides);
  let script = format!(
    "{source}; export SDKMAN_NON_INTERACTIVE=true; sdk uninstall java {c}",
    source = source,
    c = shell_escape(c)
  );
  let out = Command::new("bash")
    .args(["-lc", &script])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn java_major_from_version(version: &str) -> Option<u32> {
  let v = version.trim();
  if v.is_empty() || v == "latest" || v == "stable" {
    return None;
  }
  let mut s = v;
  if let Some(rest) = v.strip_prefix("1.") {
    s = rest;
  }
  let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
  if digits.is_empty() {
    None
  } else {
    digits.parse::<u32>().ok()
  }
}

fn homebrew_java_formula(version: &str) -> String {
  let v = version.trim();
  if v.is_empty() || v == "latest" || v == "stable" {
    return "openjdk".to_string();
  }
  let major = java_major_from_version(v).unwrap_or(21);
  format!("openjdk@{major}")
}

fn winget_java_id(major: u32) -> String {
  format!("EclipseAdoptium.Temurin.{major}.JDK")
}

fn winget_id_installed(id: &str) -> bool {
  if !cfg!(windows) {
    return false;
  }
  let out = Command::new("winget")
    .args(["list", "--id", id, "-e"])
    .output();
  let Ok(out) = out else {
    return false;
  };
  let (text, _) = merge_output(&out);
  text.lines().any(|l| l.contains(id))
}

fn homebrew_php_formula(version: &str) -> String {
  let v = version.trim();
  let parts: Vec<&str> = v.split('.').collect();
  if parts.len() >= 2 {
    return format!("php@{}.{}", parts[0], parts[1]);
  }
  if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.') {
    return format!("php@{v}");
  }
  "php".to_string()
}

fn winget_php_id(version: &str) -> String {
  let v = version.trim();
  let parts: Vec<&str> = v.split('.').collect();
  if parts.len() >= 2 {
    return format!("PHP.PHP.{}.{}", parts[0], parts[1]);
  }
  "PHP.PHP.8.4".to_string()
}

fn run_uninstall_homebrew(formula: &str) -> Result<String, String> {
  if cfg!(windows) {
    return Err("当前系统不支持 Homebrew 安装器".to_string());
  }
  let out = Command::new("brew")
    .args(["uninstall", "--ignore-dependencies", formula])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_uninstall_winget(id: &str) -> Result<String, String> {
  if !cfg!(windows) {
    return Err("当前系统不支持 winget 安装器".to_string());
  }
  let out = Command::new("winget")
    .args([
      "uninstall",
      "--id",
      id,
      "-e",
      "--accept-source-agreements",
      "--accept-package-agreements",
    ])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_uninstall_rustup(version: &str) -> Result<String, String> {
  let out = Command::new("rustup")
    .args(["toolchain", "uninstall", version])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_uninstall_fnm(version: &str) -> Result<String, String> {
  let v = if version.starts_with('v') {
    version.to_string()
  } else {
    format!("v{}", version)
  };
  let out = Command::new("fnm")
    .args(["uninstall", &v])
    .output()
    .or_else(|_| Command::new("fnm").args(["uninstall", version]).output())
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_uninstall_nvm(version: &str, overrides: &HashMap<String, String>) -> Result<String, String> {
  let v = if version.starts_with('v') {
    version.to_string()
  } else {
    format!("v{}", version)
  };
  if cfg!(windows) {
    let p = nvm_program(overrides)?;
    let out = Command::new(&p)
      .args(["uninstall", &v])
      .output()
      .or_else(|_| Command::new(&p).args(["uninstall", version]).output())
      .map_err(|e| e.to_string())?;
    return Ok(merge_output(&out).0.trim().to_string());
  }
  let script = format!(
    "export NVM_DIR=\"${{NVM_DIR:-$HOME/.nvm}}\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"; nvm uninstall {v}; nvm list",
    v = shell_escape(&v)
  );
  let out = Command::new("zsh")
    .args(["-lc", &script])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn run_uninstall_pyenv(version: &str) -> Result<String, String> {
  if !pyenv_has_uninstall()? {
    return Err("当前 pyenv 未提供 uninstall 命令，请安装 pyenv-uninstall 或手动删除版本目录".to_string());
  }
  let out = Command::new("pyenv")
    .args(["uninstall", "-f", version])
    .output()
    .map_err(|e| e.to_string())?;
  Ok(merge_output(&out).0.trim().to_string())
}

fn pyenv_has_uninstall() -> Result<bool, String> {
  let out = Command::new("pyenv")
    .args(["commands"])
    .output()
    .map_err(|e| e.to_string())?;
  let (text, _) = merge_output(&out);
  Ok(text.lines().any(|l| l.trim() == "uninstall"))
}

fn ensure_dir(dir: &Path) -> Result<(), String> {
  if dir.exists() {
    return Ok(());
  }
  fs::create_dir_all(dir).map_err(|e| e.to_string())
}
