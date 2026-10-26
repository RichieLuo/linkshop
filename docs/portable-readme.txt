使用方法
1. 完整解压压缩包；macOS / Linux 启动 linkshop。
2. 双击 linkshop.exe 启动，无需安装 Rust 或 Visual Studio。
3. 输入 http:// 或 https:// URL，点击“发送”。

常用快捷键
Ctrl+T       新建请求
Ctrl+W       关闭请求标签
Ctrl+S       保存请求
Ctrl+Enter   发送请求
Ctrl+K       搜索本地请求
Ctrl+L       聚焦 URL

请求、历史和打开的标签保存在当前用户的：
%APPDATA%\LinkShop\data\

支持参数/请求头网格、批量编辑、raw/urlencoded 请求体、Bearer/Basic/API Key、响应查看。
右键集合选择“集合身份验证”，统一设置 token；勾选批量继承可同时切换已有请求（含子文件夹）。
从集合添加的新请求默认继承；已有请求也可在“身份验证”中选择“继承集合身份验证”。更新集合 token 后，下次发送立即生效。
支持导入集合和环境 JSON，保留文件夹层级、认证和禁用行。点击“导入”或拖入 JSON。
集合、文件夹和请求支持右键菜单，可添加、打开、复制、重命名和删除；删除前确认，已打开请求保留为草稿。
请求标签支持右键保存、复制、关闭当前/其他/右侧/全部标签；批量关闭会逐个提示保存未保存的更改。
右上角选择环境；点击“变量”编辑全局、请求和环境变量，使用 {{变量名}} 引用。
脚本执行、multipart/binary 请求体、动态变量和云协作尚未实现。

命令行导入（请先关闭所有 LinkShop 窗口）：
linkshop.exe import "C:\exports\collection.json" "C:\exports\environment.json"
导入前自动备份现有工作区，结果保存在数据目录 last-import-report.json。

此包未签名。程序版本、target 和 SHA-256 见 package.json。
第三方组件信息见 THIRD-PARTY-NOTICES.tsv 和 licenses 文件夹。
