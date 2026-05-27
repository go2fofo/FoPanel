/*
 * @Author: fofo
 * @Date: 2026-05-26 15:55:06
 * @LastEditTime: 2026-05-26 16:06:44
 * @LastEditors: fofo
 * @Description: 
 * @FilePath: /FoPanel/src-tauri/src/models/runtime.rs
 */
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeVersion {
  pub language: String,
  pub version: String,
  pub path: String,
  pub active: bool,
  pub source: String,
}
