/*
 * @Author: fofo
 * @Date: 2026-05-26 15:52:33
 * @LastEditTime: 2026-05-26 16:16:55
 * @LastEditors: fofo
 * @Description: 
 * @FilePath: /FoPanel/src-tauri/src/lib.rs
 */
mod commands;
mod models;
mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      commands::python_cmds::pip_list,
      commands::runtime_cmds::ping,
      commands::runtime_cmds::scan_runtimes,
      commands::runtime_cmds::get_system_runtimes,
      commands::runtime_cmds::get_activated_runtimes,
      commands::runtime_cmds::add_manual_runtime,
      commands::runtime_cmds::activate_runtime,
      commands::runtime_cmds::get_activation_export,
      commands::runtime_cmds::get_runtime_detail,
      commands::runtime_cmds::remove_runtime,
      commands::runtime_cmds::list_installers,
      commands::runtime_cmds::install_runtime,
      commands::runtime_cmds::uninstall_runtime,
      commands::runtime_cmds::check_runtime_upgrade,
      commands::runtime_cmds::list_runtime_profiles,
      commands::runtime_cmds::upsert_runtime_profile,
      commands::runtime_cmds::delete_runtime_profile,
      commands::runtime_cmds::get_installer_status,
      commands::runtime_cmds::get_installer_bootstrap,
      commands::shell_cmds::shell_exec
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
