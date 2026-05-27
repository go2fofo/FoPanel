/*
 * @Author: fofo
 * @Date: 2026-05-26 15:54:51
 * @LastEditTime: 2026-05-26 16:07:50
 * @LastEditors: fofo
 * @Description: 
 * @FilePath: /FoPanel/src-tauri/src/services/runtime_service.rs
 */
use crate::models::runtime::RuntimeVersion;
use std::{
  collections::HashSet,
  env,
  fs,
  path::{Path, PathBuf},
  process::Command,
};

pub fn scan_system_runtimes() -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();

  out.extend(scan_python());
  out.extend(scan_node());
  out.extend(scan_bun());
  out.extend(scan_deno());
  out.extend(scan_go());
  out.extend(scan_rust());
  out.extend(scan_php());

  let mut seen = HashSet::<(String, String, String, String)>::new();
  out.retain(|r| {
    seen.insert((
      r.language.clone(),
      r.version.clone(),
      r.path.clone(),
      r.source.clone(),
    ))
  });

  out.sort_by(|a, b| a.language.cmp(&b.language).then(a.version.cmp(&b.version)));
  out
}

fn classify_exec_source(exec: &Path) -> String {
  let real = fs::canonicalize(exec).unwrap_or_else(|_| exec.to_path_buf());
  let s = real.to_string_lossy();
  if s.contains("/.volta/") {
    return "volta".to_string();
  }
  if s.contains("/.asdf/") {
    return "asdf".to_string();
  }
  if s.contains("/Cellar/") || s.starts_with("/opt/homebrew/") {
    return "homebrew".to_string();
  }
  "path".to_string()
}

fn is_pyenv_shim(exec: &Path) -> bool {
  let raw = exec.to_string_lossy();
  if raw.contains("/.pyenv/shims/") {
    return true;
  }
  let real = fs::canonicalize(exec).unwrap_or_else(|_| exec.to_path_buf());
  real.to_string_lossy().contains("/.pyenv/shims/")
}

fn scan_bun() -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let Some(exec) = find_in_path("bun") else {
    return out;
  };
  let Some(version) = read_bun_version(&exec) else {
    return out;
  };
  out.push(RuntimeVersion {
    language: "bun".to_string(),
    version,
    path: exec.to_string_lossy().to_string(),
    active: true,
    source: classify_exec_source(&exec),
  });
  out
}

fn scan_deno() -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let Some(exec) = find_in_path("deno") else {
    return out;
  };
  let Some(version) = read_deno_version(&exec) else {
    return out;
  };
  out.push(RuntimeVersion {
    language: "deno".to_string(),
    version,
    path: exec.to_string_lossy().to_string(),
    active: true,
    source: classify_exec_source(&exec),
  });
  out
}

fn scan_python() -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();

  let python3 = find_in_path("python3");
  let python = find_in_path("python");
  let mut active_exec = python3.clone().or(python.clone());
  if find_in_path("pyenv").is_some() {
    if let Some(p) = run_capture("pyenv", &["which", "python3"])
      .or_else(|| run_capture("pyenv", &["which", "python"]))
    {
      let exec = PathBuf::from(p.trim());
      if exec.exists() {
        active_exec = Some(exec);
      }
    }
  }

  for (exec, source) in [(python3, "path"), (python, "path")] {
    let Some(exec) = exec else {
      continue;
    };
    if is_pyenv_shim(&exec) {
      continue;
    }
    let Some(version) = read_python_version(&exec) else {
      continue;
    };
    out.push(RuntimeVersion {
      language: "python".to_string(),
      version,
      path: exec.to_string_lossy().to_string(),
      active: active_exec.as_ref() == Some(&exec),
      source: if source == "path" {
        classify_exec_source(&exec)
      } else {
        source.to_string()
      },
    });
  }

  out.extend(scan_python_standalone(&active_exec));
  out.extend(scan_python_asdf(&active_exec));
  out.extend(scan_python_homebrew(&active_exec));

  let home = home_dir();
  if let Some(home) = home {
    let pyenv_versions_dir = home.join(".pyenv").join("versions");
    if let Ok(entries) = fs::read_dir(&pyenv_versions_dir) {
      for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
          continue;
        }
        let exec = pick_python_exec_in_dir(&dir);
        let Some(exec) = exec else {
          continue;
        };
        let Some(version) = read_python_version(&exec) else {
          continue;
        };
        out.push(RuntimeVersion {
          language: "python".to_string(),
          version,
          path: exec.to_string_lossy().to_string(),
          active: active_exec
            .as_ref()
            .is_some_and(|p| p.starts_with(&dir)),
          source: "pyenv".to_string(),
        });
      }
    }
  }

  let framework_dir = PathBuf::from("/Library/Frameworks/Python.framework/Versions");
  if let Ok(entries) = fs::read_dir(&framework_dir) {
    for entry in entries.flatten() {
      let dir = entry.path();
      let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
        continue;
      };
      if name == "Current" {
        continue;
      }
      if !dir.is_dir() {
        continue;
      }
      let exec = dir.join("bin").join("python3");
      if !exec.exists() {
        continue;
      }
      let Some(version) = read_python_version(&exec) else {
        continue;
      };
      out.push(RuntimeVersion {
        language: "python".to_string(),
        version,
        path: exec.to_string_lossy().to_string(),
        active: active_exec
          .as_ref()
          .is_some_and(|p| p.starts_with(&dir)),
        source: "framework".to_string(),
      });
    }
  }

  out
}

fn scan_python_standalone(active_exec: &Option<PathBuf>) -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let candidates = [
    "/usr/bin/python3",
    "/usr/local/bin/python3",
    "/opt/homebrew/bin/python3",
    "/usr/bin/python",
    "/usr/local/bin/python",
    "/opt/homebrew/bin/python",
  ];
  for c in candidates {
    let p = PathBuf::from(c);
    if !p.exists() {
      continue;
    }
    if is_pyenv_shim(&p) {
      continue;
    }
    let Some(version) = read_python_version(&p) else {
      continue;
    };
    let source = classify_exec_source(&p);
    out.push(RuntimeVersion {
      language: "python".to_string(),
      version,
      path: p.to_string_lossy().to_string(),
      active: active_exec.as_ref() == Some(&p),
      source,
    });
  }
  out
}

fn scan_node() -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();

  let active_node = find_in_path("node");
  let active_version = active_node
    .as_ref()
    .and_then(|p| read_node_version(p));

  if let (Some(path), Some(version)) = (active_node.as_ref(), active_version.as_ref()) {
    out.push(RuntimeVersion {
      language: "node".to_string(),
      version: version.clone(),
      path: path.to_string_lossy().to_string(),
      active: true,
      source: classify_exec_source(path),
    });
  }

  if find_in_path("fnm").is_some() {
    if let Some(list) = run_capture("fnm", &["list"]) {
      for line in list.lines() {
        let l = line.trim();
        if l.ends_with("system") {
          continue;
        }
        let Some(ver) = extract_semver_like(l) else {
          continue;
        };
        let p = resolve_fnm_node_exec_path(&ver);
        out.push(RuntimeVersion {
          language: "node".to_string(),
          version: ver,
          path: p
            .as_ref()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_default(),
          active: active_node.as_ref().is_some_and(|a| p.as_ref() == Some(a)),
          source: "fnm".to_string(),
        });
      }
    }
  }

  if let Some(nvm_dir) = nvm_dir() {
    let versions_dir = nvm_dir.join("versions").join("node");
    if let Ok(entries) = fs::read_dir(&versions_dir) {
      for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
          continue;
        }
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
          continue;
        };
        if !name.starts_with('v') {
          continue;
        }
        let exec = dir.join("bin").join("node");
        if !exec.exists() {
          continue;
        }
        let Some(version) = read_node_version(&exec) else {
          continue;
        };
        let active = active_node
          .as_ref()
          .is_some_and(|p| p.starts_with(&dir));
        out.push(RuntimeVersion {
          language: "node".to_string(),
          version,
          path: exec.to_string_lossy().to_string(),
          active,
          source: "nvm".to_string(),
        });
      }
    }
  }

  out.extend(scan_node_volta(&active_node));
  out.extend(scan_node_asdf(&active_node));
  out.extend(scan_node_standalone(&active_node));

  out
}

fn scan_node_volta(active_node: &Option<PathBuf>) -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let Some(home) = home_dir() else {
    return out;
  };
  let root = home.join(".volta").join("tools").join("image").join("node");
  let Ok(entries) = fs::read_dir(&root) else {
    return out;
  };
  for entry in entries.flatten() {
    let dir = entry.path();
    if !dir.is_dir() {
      continue;
    }
    let exec = dir.join("bin").join("node");
    if !exec.exists() {
      continue;
    }
    let Some(version) = read_node_version(&exec) else {
      continue;
    };
    out.push(RuntimeVersion {
      language: "node".to_string(),
      version,
      path: exec.to_string_lossy().to_string(),
      active: active_node.as_ref().is_some_and(|p| p.starts_with(&dir)),
      source: "volta".to_string(),
    });
  }
  out
}

fn scan_node_asdf(active_node: &Option<PathBuf>) -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let Some(home) = home_dir() else {
    return out;
  };
  let root = home.join(".asdf").join("installs").join("nodejs");
  let Ok(entries) = fs::read_dir(&root) else {
    return out;
  };
  for entry in entries.flatten() {
    let dir = entry.path();
    if !dir.is_dir() {
      continue;
    }
    let exec = dir.join("bin").join("node");
    if !exec.exists() {
      continue;
    }
    let Some(version) = read_node_version(&exec) else {
      continue;
    };
    out.push(RuntimeVersion {
      language: "node".to_string(),
      version,
      path: exec.to_string_lossy().to_string(),
      active: active_node.as_ref().is_some_and(|p| p.starts_with(&dir)),
      source: "asdf".to_string(),
    });
  }
  out
}

fn scan_python_asdf(active_exec: &Option<PathBuf>) -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let Some(home) = home_dir() else {
    return out;
  };
  let root = home.join(".asdf").join("installs").join("python");
  let Ok(entries) = fs::read_dir(&root) else {
    return out;
  };
  for entry in entries.flatten() {
    let dir = entry.path();
    if !dir.is_dir() {
      continue;
    }
    let exec = if dir.join("bin").join("python3").exists() {
      dir.join("bin").join("python3")
    } else {
      dir.join("bin").join("python")
    };
    if !exec.exists() {
      continue;
    }
    let Some(version) = read_python_version(&exec) else {
      continue;
    };
    out.push(RuntimeVersion {
      language: "python".to_string(),
      version,
      path: exec.to_string_lossy().to_string(),
      active: active_exec.as_ref().is_some_and(|p| p.starts_with(&dir)),
      source: "asdf".to_string(),
    });
  }
  out
}

fn scan_python_homebrew(active_exec: &Option<PathBuf>) -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let roots = ["/opt/homebrew/Cellar", "/usr/local/Cellar"];
  for root in roots {
    let root = PathBuf::from(root);
    if !root.exists() {
      continue;
    }
    let Ok(entries) = fs::read_dir(&root) else {
      continue;
    };
    for entry in entries.flatten() {
      let pkg = entry.path();
      let Some(name) = pkg.file_name().and_then(|n| n.to_str()) else {
        continue;
      };
      if !(name == "python" || name.starts_with("python@")) {
        continue;
      }
      let Ok(vers) = fs::read_dir(&pkg) else {
        continue;
      };
      for v in vers.flatten() {
        let dir = v.path();
        if !dir.is_dir() {
          continue;
        }
        let exec = dir.join("bin").join("python3");
        if !exec.exists() {
          continue;
        }
        let Some(version) = read_python_version(&exec) else {
          continue;
        };
        out.push(RuntimeVersion {
          language: "python".to_string(),
          version,
          path: exec.to_string_lossy().to_string(),
          active: active_exec.as_ref().is_some_and(|p| p.starts_with(&dir)),
          source: "homebrew".to_string(),
        });
      }
    }
  }
  out
}

fn resolve_fnm_node_exec_path(version: &str) -> Option<PathBuf> {
  let v = if version.starts_with('v') {
    version.to_string()
  } else {
    format!("v{}", version)
  };
  let using_arg = format!("--using={}", v);
  let out = run_capture(
    "fnm",
    &["exec", &using_arg, "node", "-p", "process.execPath"],
  )?;
  let p = out.trim();
  if p.is_empty() {
    None
  } else {
    Some(PathBuf::from(p))
  }
}

fn scan_node_standalone(active_node: &Option<PathBuf>) -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let candidates = [
    "/usr/local/bin/node",
    "/opt/homebrew/bin/node",
    "/usr/bin/node",
  ];
  for c in candidates {
    let p = PathBuf::from(c);
    if !p.exists() {
      continue;
    }
    let Some(version) = read_node_version(&p) else {
      continue;
    };
    let source = if c.starts_with("/opt/homebrew") || c.contains("/Cellar/") {
      "homebrew"
    } else {
      "standalone"
    };
    out.push(RuntimeVersion {
      language: "node".to_string(),
      version,
      path: p.to_string_lossy().to_string(),
      active: active_node.as_ref() == Some(&p),
      source: source.to_string(),
    });
  }

  let roots = [
    "/opt/homebrew/Cellar/node",
    "/usr/local/Cellar/node",
    "/usr/local/lib/nodejs",
    "/opt/nodejs",
  ];
  for root in roots {
    let root = PathBuf::from(root);
    if !root.exists() {
      continue;
    }
    out.extend(find_node_bins_in_root(&root, 5, active_node));
  }

  out
}

fn find_node_bins_in_root(
  root: &Path,
  max_depth: usize,
  active_node: &Option<PathBuf>,
) -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let mut stack = Vec::<(PathBuf, usize)>::new();
  stack.push((root.to_path_buf(), 0));

  while let Some((dir, depth)) = stack.pop() {
    if depth > max_depth {
      continue;
    }
    let Ok(entries) = fs::read_dir(&dir) else {
      continue;
    };
    for entry in entries.flatten() {
      let p = entry.path();
      if p.is_dir() {
        stack.push((p, depth + 1));
        continue;
      }
      if p.file_name().and_then(|n| n.to_str()) != Some("node") {
        continue;
      }
      if p.parent().and_then(|x| x.file_name()).and_then(|n| n.to_str()) != Some("bin") {
        continue;
      }
      let Some(version) = read_node_version(&p) else {
        continue;
      };
      let source = if p.to_string_lossy().contains("/Cellar/") {
        "homebrew"
      } else {
        "standalone"
      };
      out.push(RuntimeVersion {
        language: "node".to_string(),
        version,
        path: p.to_string_lossy().to_string(),
        active: active_node.as_ref() == Some(&p),
        source: source.to_string(),
      });
    }
  }

  out
}

fn scan_go() -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let Some(exec) = find_in_path("go") else {
    return out;
  };
  let Some(version) = read_go_version(&exec) else {
    return out;
  };
  out.push(RuntimeVersion {
    language: "go".to_string(),
    version,
    path: exec.to_string_lossy().to_string(),
    active: true,
    source: "path".to_string(),
  });
  out
}

fn scan_rust() -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let Some(exec) = find_in_path("rustc") else {
    return out;
  };
  let Some(version) = read_rustc_version(&exec) else {
    return out;
  };
  out.push(RuntimeVersion {
    language: "rust".to_string(),
    version,
    path: exec.to_string_lossy().to_string(),
    active: true,
    source: "path".to_string(),
  });
  out
}

fn scan_php() -> Vec<RuntimeVersion> {
  let mut out = Vec::<RuntimeVersion>::new();
  let Some(exec) = find_in_path("php") else {
    return out;
  };
  let Some(version) = read_php_version(&exec) else {
    return out;
  };
  out.push(RuntimeVersion {
    language: "php".to_string(),
    version,
    path: exec.to_string_lossy().to_string(),
    active: true,
    source: "path".to_string(),
  });
  out
}

fn read_python_version(exec: &Path) -> Option<String> {
  let out = run_capture(exec.to_string_lossy().as_ref(), &["--version"])?;
  normalize_python_version(&out)
}

fn read_node_version(exec: &Path) -> Option<String> {
  let out = run_capture(exec.to_string_lossy().as_ref(), &["-v"])?;
  normalize_node_version(&out)
}

fn read_go_version(exec: &Path) -> Option<String> {
  let out = run_capture(exec.to_string_lossy().as_ref(), &["version"])?;
  let s = out.trim();
  let token = s.split_whitespace().find(|x| x.starts_with("go"))?;
  Some(token.trim_start_matches("go").to_string())
}

fn read_rustc_version(exec: &Path) -> Option<String> {
  let out = run_capture(exec.to_string_lossy().as_ref(), &["--version"])?;
  let s = out.trim();
  extract_semver_like(s)
}

fn read_php_version(exec: &Path) -> Option<String> {
  let out = run_capture(exec.to_string_lossy().as_ref(), &["-v"])?;
  let first = out.lines().next()?.trim();
  extract_semver_like(first)
}

fn read_bun_version(exec: &Path) -> Option<String> {
  let out = run_capture(exec.to_string_lossy().as_ref(), &["--version"])?;
  let s = out.trim();
  extract_semver_like(s).or_else(|| Some(s.to_string()))
}

fn read_deno_version(exec: &Path) -> Option<String> {
  let out = run_capture(exec.to_string_lossy().as_ref(), &["--version"])?;
  for line in out.lines() {
    let l = line.trim();
    if l.starts_with("deno ") || l == "deno" {
      return extract_semver_like(l);
    }
  }
  let first = out.lines().next()?.trim();
  extract_semver_like(first)
}

fn normalize_python_version(raw: &str) -> Option<String> {
  let s = raw.trim();
  if let Some(rest) = s.strip_prefix("Python ") {
    return Some(rest.trim().to_string());
  }
  extract_semver_like(s)
}

fn normalize_node_version(raw: &str) -> Option<String> {
  let s = raw.trim();
  let s = s.strip_prefix('v').unwrap_or(s);
  extract_semver_like(s).or_else(|| Some(s.to_string()))
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

fn run_capture(program: &str, args: &[&str]) -> Option<String> {
  let output = Command::new(program).args(args).output().ok()?;
  let mut text = String::new();
  if !output.stdout.is_empty() {
    text.push_str(&String::from_utf8_lossy(&output.stdout));
  }
  if !output.stderr.is_empty() {
    if !text.is_empty() {
      text.push('\n');
    }
    text.push_str(&String::from_utf8_lossy(&output.stderr));
  }
  if text.trim().is_empty() {
    None
  } else {
    Some(text)
  }
}

fn find_in_path(program: &str) -> Option<PathBuf> {
  let path = env::var_os("PATH")?;
  for dir in env::split_paths(&path) {
    let candidate = dir.join(program);
    if is_executable(&candidate) {
      return Some(candidate);
    }
  }
  None
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
  false
}

fn home_dir() -> Option<PathBuf> {
  env::var_os("HOME").map(PathBuf::from)
}

fn nvm_dir() -> Option<PathBuf> {
  if let Some(dir) = env::var_os("NVM_DIR") {
    let p = PathBuf::from(dir);
    if p.exists() {
      return Some(p);
    }
  }
  let home = home_dir()?;
  let p = home.join(".nvm");
  if p.exists() {
    Some(p)
  } else {
    None
  }
}

fn pick_python_exec_in_dir(dir: &Path) -> Option<PathBuf> {
  let c1 = dir.join("bin").join("python3");
  if c1.exists() {
    return Some(c1);
  }
  let c2 = dir.join("bin").join("python");
  if c2.exists() {
    return Some(c2);
  }
  None
}
