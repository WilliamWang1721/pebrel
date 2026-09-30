# Pebrel 六个 PR 的合并准备报告

修复已普通推送，六个分支均已同步到上游 2.1.0 的 main `077595b4a4f1bda0be57e52324343785f69c5a3e`。六个最终提交的 10 项必需检查全部通过；尚未达到 100% Ready to Merge，仍有维护者审批、Draft 状态和通知原生验收门槛。没有 merge PR 或 force push。记录时间：2026-09-30 18:26:52 UTC。

| PR | 当前提交 | 必需检查 | 正式记录 |
| --- | --- | --- | --- |
| [#128](https://github.com/Kuddev/pebrel/pull/128) macOS 便携启动 | `c99efb4f8a009e7c6e36d7d3b2cc6eee44cfd38c` | 10/10 全部通过 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36749669526) |
| [#280](https://github.com/Kuddev/pebrel/pull/280) SSH 本地端口转发 | `f5ab21af665c73b40809fc1a82053123397483e6` | 10/10 全部通过 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36750270127) |
| [#288](https://github.com/Kuddev/pebrel/pull/288) macOS 原生菜单 | `571e0a388558d294439c1788b1200c59aef19e71` | 10/10 全部通过 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36750404192) |
| [#289](https://github.com/Kuddev/pebrel/pull/289) macOS 前台系统通知 | `96ed7d3b18f6aff0c27baed22c6d9330638ccb7f` | 10/10 全部通过 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36750433708) |
| [#291](https://github.com/Kuddev/pebrel/pull/291) 用户文档站点 | `81418821a638fbf8675054bf4b908a11a138f653` | 10/10 全部通过 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36750468358) |
| [#294](https://github.com/Kuddev/pebrel/pull/294) 自定义更新来源 | `edb18803dc308f56479e1155b481deca49c4ad48` | 10/10 全部通过 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36750507475) |

## 修复与代码复查

- #128：保留当前 main 的启动、completion、mobile、plugin 和 login 行为，修复 daemon 与 App Translocation 判断。原生验收发现点击便携启动后 GPUI 初始化崩溃；改用已锁定的 Core Foundation 原生提示，避免 RFD 提前创建普通 NSApplication。
- #280：保留当前 SSH transcript 模块，修复合并冲突；核对 listener 重试、真实 SSH 通道拒绝、64 个并发任务上限，以及取消、退出和清理。复用 SSH 传输与现有对话框。
- #288：复用已有退出动作，明确 ⌘W 关闭窗口、⌘⇧W 关闭终端、⌘Q 退出；编辑菜单复用 GPUI 输入动作，About 入口选择应用设置首页。修复旧 keymap 与 Quit 默认绑定的冲突。
- #289：复用现有系统通知派发器，为已锁定的 macOS delegate 补齐前台呈现回调。可见性验收仍未通过，不能以派发成功代替实际显示。
- #291：核对无 JavaScript 的移动导航、生成目录清理保护、实际安装包名及贡献指南。保留等待期间新增的补全、加密备份、Android 配对指南，并核对逐页来源版本与操作步骤。
- #294：保留发行渠道的更新限制；修复过期后台结果、来源重置与 About 链接。实际输入、键盘替换、校验、保存、重开和清空的回归与主线新主题编辑测试共同保留。

17 条 Copilot 评论线程全部已解决，问题修复已对照代码核实。评论以具体问题和修改建议为主，属于建设性的审查；COMMENTED 不等于 APPROVED。

已完成正确性与最小性两轮独立复查，每次主线同步后重新核对最终差异。最终本地树与发布树一致，工作区干净；版本、翻译完整性与重复键检查通过，格式、空白和架构检查通过。没有修改合并规则或把 PR 专用验收工作流加入产品差异。#291 的最终文档构建与桌面/移动浏览器检查通过：[文档 CI](https://github.com/Kuddev/pebrel/actions/runs/36750468254)；本地 9 项文档测试通过。

## 最终提交的原生证据

[#128、#288 原生验收 run 36750717366](https://github.com/WilliamWang1721/pebrel/actions/runs/36750717366)均通过，结构化结果已下载复查：

- #128 `c99efb4`：真实点击三种启动选择、便携配置、驻留端点与 CLI、正常退出、移动应用和数据后重启。四个进程退出码均为 0，未出现 GPUI 初始化崩溃。
- #288 `571e0a3`：原生 New tab、创建窗口、⌘⇧W 关闭终端、⌘W 关闭窗口和 ⌘Q 正常退出。

#280 的 macOS 必需报告按既有路径策略汇报成功，没有实际执行 macOS 工作负载；本 PR 的实际原生作业覆盖 Linux、Windows x64 和 Windows ARM。不会把路径选择结果描述成五个平台都执行了测试。

[#289 权限流程重测 run 36753918280](https://github.com/WilliamWang1721/pebrel/actions/runs/36753918280)绑定最终提交 `96ed7d3`，仍未观察到通知。测试 helper 的 bundle ID 正确；初始授权 0（未决定），申请超时后为 1（拒绝），系统设置授权步骤未成功。注册、派发、前台 policy 安装和激活均未报告错误。这不能证明系统实际显示成功，也不足以确定归因于操作系统。此项仍为验收阻塞。

最后核对：全部必需状态来自规则指定的 GitHub Actions 集成（15368）；六个 PR 的 head 与已验证提交一致，均无冲突，目标 main 仍为 `077595b`。正式 CI 没有未完成或失败的必需项。

## 剩余合并门槛

1. main 要求 Kuddev 的一次新 code-owner 批准；作者不能代替，提交更新会撤销旧审批。六个 PR 目前均没有有效新批准，#280 的前次批准已被撤销。
2. #288 仍是 Draft。当前 GitHub 连接修改上游 PR、发布验证评论和转换 Ready 均返回 403/FORBIDDEN；因此上游 PR 描述仍含较早提交的验证信息。
3. #289 系统通知实际显示还需完成原生界面诊断和验收。

已在本地准备截图诊断方案：只重测 #289 的上述最终提交，使用全新的 macOS GitHub runner 和合成配置，将测试 App 的 PNG 截图及合成 acceptance.json 上传至公开 fork，保留 7 天，不上传原始日志。尚未执行公开截图上传。

自动审批此前拒绝重新打包公开原始日志和截图，理由是可能包含 runner 路径、命令或环境信息。已请求用户明确授权，尚未收到答复；被拒绝的产物没有重新导出。后续验收只导出了合成检查名称、布尔值和数值诊断。

本轮由 Codex（GPT-6）协助完成；具体模型 variant 和 reasoning effort 未在当前运行环境提供。
