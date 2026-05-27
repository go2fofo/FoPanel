#[tauri::command]
pub fn shell_exec(_program: String, _args: Vec<String>) -> Result<String, String> {
  Err("not implemented".to_string())
}
