
# 项目名： FoPanel（跨语言运行时管理器）

## 1. 宏观意图 (Proposal)

* **愿景：** 构建一个一体化的桌面工作台，统一管理 Python, Node.js, Go, Rust, PHP 等主流开发环境。
* **核心痛点：** 解决多语言工具链（如 nvm, pyenv, asdf 等）在终端操作中的碎片化问题，提供可视化、低认知负载的安装、切换与依赖管理体验。
* **核心价值：** 实现全语言的一键式环境部署、版本快速切换以及对 Python `pip` 等工具的深层次集成管理。

---

## 2. 开发环境准备 (Prerequisites)

已知当前开发环境为：
当前电脑开发环境问题需要你记忆一下，如下node管理，fnm，在执行之前可以fnm use v22.18.0，**已经安装了pnpm**
Rust环境已经支持，**已经安装了rustup**


---

## 3. 功能规格 (Functional Specs)

### 3.1 运行时矩阵管理 (Runtime Matrix)

* **动态安装器：** 实现针对各语言（Python, Node 等）的标准化安装接口。
* **版本切换引擎：** 核心功能，通过修改或软链接系统环境变量（PATH）实现版本无缝切换。
* **状态同步：** 检测并标记当前系统默认版本与应用内激活版本。

### 3.2 Python 深度集成 (Python Deep Dive)

* **依赖可视化：** 交互式管理 `pip` 依赖（列表、安装、更新、卸载）。
* **虚拟环境 (venv) 控制：** 支持为不同项目创建隔离的虚拟环境，防止全局污染。
* **快照管理：** 支持一键导出/导入 `requirements.txt`。

---

## 4. 目录结构设计 (Scaffolding)

```text
fopanel/
├── src/                        # 前端 React/Vue 源码
│   ├── features/               # 按语言划分的业务模块
│   │   ├── python/             # pip/venv 管理器界面
│   │   ├── node/               # node 版本管理逻辑
│   │   └── runtime/            # 核心版本切换器
│   └── components/             # 公共 UI 组件
├── src-tauri/                  # 后端 Rust 核心
│   ├── src/
│   │   ├── commands/           # IPC 命令接口
│   │   │   ├── python_cmds.rs  # pip & venv Rust 后端实现
│   │   │   ├── runtime_cmds.rs # 版本扫描与切换逻辑
│   │   │   └── shell_cmds.rs   # 封装系统 shell 调用
│   │   ├── services/           # 业务逻辑服务层 (扫描、解析)
│   │   └── models/             # 定义通用 Runtime 对象结构
│   ├── capabilities/           # Tauri 2.0 权限安全配置
│   └── tauri.conf.json         # 项目配置文件
├── package.json                # 前端依赖与脚本
└── Cargo.toml                  # Rust 依赖与配置

```

---

## 5. 开发路线图 (Roadmap)

1. **Phase 1 (基础框架)：** 搭建 Tauri 2.0 环境，完成项目骨架。
2. **Phase 2 (环境探测)：** 实现 Rust 后端对系统中已安装语言版本（Python, Node）的自动扫描。
3. **Phase 3 (核心功能)：** 实现多版本间的切换切换逻辑，并优先实现 Python 的 `pip` 可视化管理。
4. **Phase 4 (扩展与优化)：** 加入 Go, Rust, PHP 支持，完善 SQLite 数据持久化与系统托盘功能。

---

### 给开发的特别提示 (Pro-Tips)

* **安全性 (Security)：** 请在 `tauri.conf.json` 中配置严谨的 `capabilities`。由于涉及 `PATH` 修改，请确保你的 `shell` 插件权限仅限于调用特定路径下的可执行文件，防止应用被恶意代码滥用。
* **Python 处理：** 建议后端通过调用 `python -m pip` 来执行依赖操作，这样可以确保操作的是你当前切换到的 Python 解释器对应的 pip，而不是系统默认的那个。
