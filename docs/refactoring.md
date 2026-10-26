# 本轮结构调整

本轮保留原业务公共接口、数据格式和配置目录，拆开混合职责与测试。统计为物理行数，包含注释和空行；以下是原大文件与现在入口的对照，不代表删除了这些功能。

| 原文件 | 调整前 | 现在入口 | 调整后 |
| --- | ---: | --- | ---: |
| `src/ui.rs` | 3526 | `src/ui/mod.rs` | 889 |
| `src/model.rs` | 714 | `src/model.rs` | 569 |
| `src/import.rs` | 934 | `src/import.rs` | 497 |

- `src/ui/mod.rs`：UI 入口保留状态与生命周期；shell、editor、dialogs、actions、theme 和 widgets 分别处理导航、请求响应、窗口、用户动作、样式与控件。测试按认证、集合、标签与布局分组。

- `src/model.rs`：请求模型与参数/变量行为留在模型中，HTTP 执行与请求变量展开移入 model/transport.rs。公共调用路径保持不变。

- `src/import.rs`：格式解析与本机 profile 文件读取、备份、合并、命令行导入分开。新建 lib.rs 供桌面入口和预览示例共同使用，不再重复编译私有业务模块。

所有新增子模块通过正常的 Rust 模块声明连接，不使用 include! 拼接源码。测试仍由 cargo test 自动发现；忽略的实库/显式样本测试保留原忽略条件。

运行 `python scripts/check-structure.py` 查看当前最大文件与 CI 检查线。vendor 驱动补丁不做纯风格重排，以保留上游差异可审阅性。

## 验证记录（2026-10-25，Windows x64）

- `cargo fmt --all --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked` 通过；原有显式忽略的实库/调用方样本测试仍按原条件跳过。
- `python -m unittest discover -s scripts/tests -v` 的 7 个打包契约测试通过，覆盖失败构建、目标目录、归档权限、静态 CRT 参数、旧包保护和版本 tag 匹配。
- `python scripts/check-structure.py` 与 `git diff --check` 通过。
- 对照 Git 原版本核对函数声明，未发现原有函数遗漏；24 个 Rust 测试声明保持一致。SQLShop 的图标像素函数统一命名为 `icon_image`。
- Windows 发布包实际构建完成，ZIP 完整性、包内程序 SHA-256、使用说明和第三方许可清单核对通过；PE 版本资源与项目名称/版本一致，依赖列表不包含动态 MSVC CRT。
- 三仓库的通用打包脚本、资源环境脚本、文件规模检查、发布工作流、架构文档及格式配置保持一致。

本次生成的候选包为 `dist/architecture-unification/LinkShop-0.2.4-windows-x64.zip`，SHA-256 为 `08fb4369100edd5a1565a0bd00c4a8b2166de088a83d1ecbb749aa338b92e771`。此目录是验证输出；常规打包仍默认使用 `dist/`，已有同名产物不会被覆盖。

本轮未执行 GitHub Actions、macOS/Linux 构建或目标系统启动验证。跨平台 CI 和发布配置已统一，不能将本机 Windows 验证视为三系统实测。
