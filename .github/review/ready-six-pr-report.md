# Pebrel 六个 PR 的合并准备报告

六个分支已通过普通 merge commit 同步至 main `8cedb51f2fd7957d7b05c8b942d5fa18e4e41c5a`。当前提交的必需 CI 正在运行，尚无失败；#280 的 10 项必需检查已全部通过；上一轮提交的 60 项必需检查通过不能替代本轮结果。尚有维护者审批、#288 Draft 和 #289 通知可见性验收。记录时间：2026-10-01 10:37 Asia/Singapore。

| PR | 当前提交 | 必需检查 | 正式记录 |
| --- | --- | --- | --- |
| [#128](https://github.com/Kuddev/pebrel/pull/128) macOS 便携启动 | `871e16f358c85ea63106c1f1442fa4ad8f71dfe4` | 正在运行 | 当前 head 的正式检查待汇总 |
| [#280](https://github.com/Kuddev/pebrel/pull/280) SSH 本地端口转发 | `c7120180727cdc16598a70309ca59ce3564c148d` | 10/10 全部通过 | 当前 head 的必需检查均来自 GitHub Actions |
| [#288](https://github.com/Kuddev/pebrel/pull/288) macOS 原生菜单 | `d32afe3f0e45dacb94d39ac522d597c6a432fa6d` | 正在运行 | 当前 head 的正式检查待汇总 |
| [#289](https://github.com/Kuddev/pebrel/pull/289) macOS 前台系统通知 | `8193eeffee5a39bfb137eefbf671e6406ad1dd9e` | 正在运行 | 当前 head 的正式检查待汇总 |
| [#291](https://github.com/Kuddev/pebrel/pull/291) 用户文档站点 | `ae87ea4e3ff4c3ed1a37415592c5410cab486a1f` | 正在运行 | 当前 head 的正式检查待汇总 |
| [#294](https://github.com/Kuddev/pebrel/pull/294) 自定义更新来源 | `f27044cb968425d40dea25ea87f75c1ebb7aa601` | 正在运行 | 当前 head 的正式检查待汇总 |

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

#289 已在用户明确授权后执行公开截图验收，只上传合成测试界面的 PNG 和 acceptance.json，保留 7 天；不导出原始日志。最新 run [36805607745](https://github.com/WilliamWang1721/pebrel/actions/runs/36805607745) 绑定当前提交 `8193eeffee5a39bfb137eefbf671e6406ad1dd9e`，未观察到系统通知。修正探针后权限保持 0（未决定），此前申请超时后的“拒绝”是探针造成，不能作为产品归因证据。截图还揭示测试的 AX 滚动和时钟点击未真正打开目标页面，真实 GUI 输入重测 [36806536236](https://github.com/WilliamWang1721/pebrel/actions/runs/36806536236) 已确认通知设置滚动到底，但仍无 Pebrel 条目、授权仍未决定。独立候选分支复用已锁定 GPUI 的现代通知服务，验收 [36807123403](https://github.com/WilliamWang1721/pebrel/actions/runs/36807123403) 正在运行，尚未更新产品 PR。注册和派发无错误均不代替系统显示成功。

## 剩余合并门槛

1. main 要求 Kuddev 的一次新 code-owner 批准；作者不能代替，提交更新会撤销旧审批。六个 PR 目前均没有有效新批准，#280 的前次批准已被撤销。
2. #288 仍是 Draft。当前 GitHub 连接修改上游 PR、发布验证评论和转换 Ready 均返回 403/FORBIDDEN；因此上游 PR 描述仍含较早提交的验证信息。
3. #289 系统通知实际显示还需完成原生界面诊断和验收。

用户已于本轮明确同意 PNG 与合成 JSON 的公开上传。原始日志不在授权范围内，未重新导出。

本轮由 Codex（GPT-6）协助完成；具体模型 variant 和 reasoning effort 未在当前运行环境提供。
