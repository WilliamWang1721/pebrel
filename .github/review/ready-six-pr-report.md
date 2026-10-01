# Pebrel open PR 合并准备报告

七个 PR 已通过普通 merge commit 同步至 main `894e7540e63e29d46ecf9ff936cabea16ea99f9e`。#280、#291 的最新提交已通过 10 项必需检查，其余同步后的检查仍在运行。旧提交的绿色结果不替代本轮结果。维护者审批、#288 Draft 和 #289 通知可见性验收仍是合并门槛。记录时间：2026-10-01 12:34 Asia/Singapore。

| PR | 当前提交 | 必需检查 | 正式记录 |
| --- | --- | --- | --- |
| [#128](https://github.com/Kuddev/pebrel/pull/128) macOS 便携启动 | `9659e8fc763cf47462e98787503e868f41fdea3b` | 主线同步后 CI 正在运行 | 当前 head 的必需检查均来自 GitHub Actions |
| [#280](https://github.com/Kuddev/pebrel/pull/280) SSH 本地端口转发 | `f2bb7e463c50f52d7fd643df42b8f79aedba6edd` | 10/10 全部通过 | 当前 head 的必需检查均来自 GitHub Actions |
| [#288](https://github.com/Kuddev/pebrel/pull/288) macOS 原生菜单 | `94dfdf320af0942ed9fb9e271a05b1a0cda270f3` | 主线同步后 CI 正在运行 | 当前 head 的必需检查均来自 GitHub Actions |
| [#289](https://github.com/Kuddev/pebrel/pull/289) macOS 前台系统通知 | `0d454f441e3b26a128076383c6513be3b94d66c6` | 主线同步后 CI 正在运行 | 当前 head 的必需检查均来自 GitHub Actions |
| [#291](https://github.com/Kuddev/pebrel/pull/291) 用户文档站点 | `5656fab43b139077ca28cfa28fc08681a100e989` | 10/10 全部通过 | 当前 head 的必需检查均来自 GitHub Actions |
| [#294](https://github.com/Kuddev/pebrel/pull/294) 自定义更新来源 | `2b7d5aaa187c1ab6825b865c90746fa71fbea8d6` | 主线同步后 CI 正在运行 | 当前 head 的必需检查均来自 GitHub Actions |
| [#418](https://github.com/Kuddev/pebrel/pull/418) 安装后更新说明 | `e763523db8ad0bbe3a6d053cd11b1e46ee682adf` | 主线同步后 CI 正在运行 | 新增 PR，已纳入检查 |

## 修复与代码复查

- #128：保留当前 main 的启动、completion、mobile、plugin 和 login 行为，修复 daemon 与 App Translocation 判断。原生验收发现点击便携启动后 GPUI 初始化崩溃；改用已锁定的 Core Foundation 原生提示，避免 RFD 提前创建普通 NSApplication。
- #280：保留当前 SSH transcript 模块，修复合并冲突；核对 listener 重试、真实 SSH 通道拒绝、64 个并发任务上限，以及取消、退出和清理。复用 SSH 传输与现有对话框。
- #288：复用已有退出动作，明确 ⌘W 关闭窗口、⌘⇧W 关闭终端、⌘Q 退出；编辑菜单复用 GPUI 输入动作，About 入口选择应用设置首页。修复旧 keymap 与 Quit 默认绑定的冲突。
- #289：当前 PR 的旧 backend 前台 delegate 补丁未通过可见性验收。独立候选改用已锁定 GPUI 的原生 UserNotifications 服务；候选 `4af7fad` 编译通过，系统权限控件已验证可授予授权。测试桌面原先未启动原生通知 UI 服务；通过该临时用户会话的正式 launchctl 操作启动已安装服务后，已看到系统设置通知，但run 36813963715 的截图已证明 Pebrel 通知正文实际可见，最终提交的自动验收仍在运行。实现已更新到产品 PR `0d454f441e3b26a128076383c6513be3b94d66c6`，正式 CI 与最终提交的原生验收正在运行。
- #291：核对无 JavaScript 的移动导航、生成目录清理保护、实际安装包名及贡献指南。保留等待期间新增的补全、加密备份、Android 配对指南，并核对逐页来源版本与操作步骤。
- #294：保留发行渠道的更新限制；修复过期后台结果、来源重置与 About 链接。实际输入、键盘替换、校验、保存、重开和清空的回归与主线新主题编辑测试共同保留。

- #418：新 PR 在本轮被发现。复查发现隐藏驻留窗口会提前消耗一次性展示标记；已推送等待可见窗口再展示的修复，继续复用既有带锁的更新状态。新回归在独立进程测试真实隐藏/显示事件。第一次 CI 揭示新子模块对话框路径漏改，已修正并格式化；当前 head 的正式 CI 尚待完成。

17 条 Copilot 评论线程全部已解决，问题修复已对照代码核实。评论以具体问题和修改建议为主，属于建设性的审查；COMMENTED 不等于 APPROVED。

已完成正确性与最小性两轮独立复查，每次主线同步后重新核对最终差异。最终通知和更新说明的本地验证树与发布树一致；版本、翻译完整性与重复键检查通过，格式、空白和架构检查通过。没有修改合并规则或把 PR 专用验收工作流加入产品差异。#291 的最终文档构建与桌面/移动浏览器检查通过：[文档 CI](https://github.com/Kuddev/pebrel/actions/runs/36750468254)；本地 9 项文档测试通过。

## 最终提交的原生证据

[#128、#288 原生验收 run 36750717366](https://github.com/WilliamWang1721/pebrel/actions/runs/36750717366)均通过，结构化结果已下载复查：

- #128 `c99efb4`：真实点击三种启动选择、便携配置、驻留端点与 CLI、正常退出、移动应用和数据后重启。四个进程退出码均为 0，未出现 GPUI 初始化崩溃。
- #288 `571e0a3`：原生 New tab、创建窗口、⌘⇧W 关闭终端、⌘W 关闭窗口和 ⌘Q 正常退出。

#280 的 macOS 必需报告按既有路径策略汇报成功，没有实际执行 macOS 工作负载；本 PR 的实际原生作业覆盖 Linux、Windows x64 和 Windows ARM。不会把路径选择结果描述成五个平台都执行了测试。

#289 已在用户明确授权后执行公开截图验收，只上传合成测试界面的 PNG 和 acceptance.json，保留 7 天；不导出原始日志。最新 run [36805607745](https://github.com/WilliamWang1721/pebrel/actions/runs/36805607745) 绑定当前提交 `8193eeffee5a39bfb137eefbf671e6406ad1dd9e`，未观察到系统通知。修正探针后权限保持 0（未决定），此前申请超时后的“拒绝”是探针造成，不能作为产品归因证据。截图还揭示测试的 AX 滚动和时钟点击未真正打开目标页面，真实 GUI 输入重测 [36806536236](https://github.com/WilliamWang1721/pebrel/actions/runs/36806536236) 已确认通知设置滚动到底，但仍无 Pebrel 条目、授权仍未决定。独立候选分支复用已锁定 GPUI 的现代通知服务，初次候选验收 [36807123403](https://github.com/WilliamWang1721/pebrel/actions/runs/36807123403) 未通过可见性；测试导航修正仍在进行。此前 [36810102750](https://github.com/WilliamWang1721/pebrel/actions/runs/36810102750) 验证合成权限控件与系统显示，尚未更新产品 PR。最新候选 `4af7fad5adaaaedf48bdc89859916ea004594525` 保留 GPUI/legacy 实际运行模式选择，避免 GPUI 初始化旧后端的 NSBundle hook。Run [36812969445](https://github.com/WilliamWang1721/pebrel/actions/runs/36812969445) 已成功启动 macOS 26 的 `com.apple.notificationcenterui.agent` 和 `com.apple.UserNotificationCenterAgent`，截图显示原生系统提示；测试授权开关被另一条系统提示遮挡，已修正导航。Run [36813963715](https://github.com/WilliamWang1721/pebrel/actions/runs/36813963715) 的截图已显示真正的 Pebrel 原生通知正文，授权为 2；测试因辅助功能树未暴露文字而失败。最终提交验收 [36814505630](https://github.com/WilliamWang1721/pebrel/actions/runs/36814505630) 的截图也已显示原生 Pebrel 通知，但其窄窗口检查漏掉 macOS 26 的透明全屏通知窗口，因此失败。修正后的 [36815104245](https://github.com/WilliamWang1721/pebrel/actions/runs/36815104245) 已通过系统通知进程窗口截取与通知正文识别，包括先切至 Finder 后通知仍可见；点击激活检查失败。当前 [36815676201](https://github.com/WilliamWang1721/pebrel/actions/runs/36815676201) 使用正式 GUI session mouse event 区分 AX 点击未送达与产品回调问题；最终验收结果仍待确认。注册和派发无错误均不代替系统显示成功。

## 剩余合并门槛

1. main 要求 Kuddev 的一次新 code-owner 批准；作者不能代替，提交更新会撤销旧审批。目前原六个 PR 和新增 #418 均没有有效新批准，#280 的前次批准已被撤销。
2. #288 仍是 Draft；本轮再次尝试转换 Ready 返回 FORBIDDEN / Resource not accessible by integration。当前 GitHub 连接修改上游 PR、发布验证评论和转换 Ready 均返回 403/FORBIDDEN；因此上游 PR 描述仍含较早提交的验证信息。
3. 等待当前七个提交的所有必需检查，以及 #289 最终提交的原生界面与点击验收。

用户已于本轮明确同意 PNG 与合成 JSON 的公开上传。原始日志不在授权范围内，未重新导出。

本轮由 Codex（GPT-6）协助完成；具体模型 variant 和 reasoning effort 未在当前运行环境提供。
