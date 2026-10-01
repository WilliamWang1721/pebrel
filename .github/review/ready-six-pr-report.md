# Pebrel open PR 合并准备报告

七个 PR 已通过普通 merge commit 同步至 main `894e7540e63e29d46ecf9ff936cabea16ea99f9e`。#280、#291、#418 的最新提交已通过 10 项必需检查，其余同步后的检查仍在运行。旧提交的绿色结果不替代本轮结果。正式 CI、维护者审批及 #288 Draft 仍是合并门槛；#289 最终提交原生显示和点击验收已通过。记录时间：2026-10-01 12:42 Asia/Singapore。

| PR | 当前提交 | 必需检查 | 正式记录 |
| --- | --- | --- | --- |
| [#128](https://github.com/Kuddev/pebrel/pull/128) macOS 便携启动 | `9659e8fc763cf47462e98787503e868f41fdea3b` | 主线同步后 CI 正在运行 | 当前 head 的必需检查均来自 GitHub Actions |
| [#280](https://github.com/Kuddev/pebrel/pull/280) SSH 本地端口转发 | `f2bb7e463c50f52d7fd643df42b8f79aedba6edd` | 10/10 全部通过 | 当前 head 的必需检查均来自 GitHub Actions |
| [#288](https://github.com/Kuddev/pebrel/pull/288) macOS 原生菜单 | `94dfdf320af0942ed9fb9e271a05b1a0cda270f3` | 主线同步后 CI 正在运行 | 当前 head 的必需检查均来自 GitHub Actions |
| [#289](https://github.com/Kuddev/pebrel/pull/289) macOS 前台系统通知 | `0d454f441e3b26a128076383c6513be3b94d66c6` | 主线同步后 CI 正在运行 | 当前 head 的必需检查均来自 GitHub Actions |
| [#291](https://github.com/Kuddev/pebrel/pull/291) 用户文档站点 | `5656fab43b139077ca28cfa28fc08681a100e989` | 10/10 全部通过 | 当前 head 的必需检查均来自 GitHub Actions |
| [#294](https://github.com/Kuddev/pebrel/pull/294) 自定义更新来源 | `2b7d5aaa187c1ab6825b865c90746fa71fbea8d6` | 主线同步后 CI 正在运行 | 当前 head 的必需检查均来自 GitHub Actions |
| [#418](https://github.com/Kuddev/pebrel/pull/418) 安装后更新说明 | `e763523db8ad0bbe3a6d053cd11b1e46ee682adf` | 10/10 全部通过 | 新增 PR，已纳入检查 |

## 修复与代码复查

- #128：保留当前 main 的启动、completion、mobile、plugin 和 login 行为，修复 daemon 与 App Translocation 判断。原生验收发现点击便携启动后 GPUI 初始化崩溃；改用已锁定的 Core Foundation 原生提示，避免 RFD 提前创建普通 NSApplication。
- #280：保留当前 SSH transcript 模块，修复合并冲突；核对 listener 重试、真实 SSH 通道拒绝、64 个并发任务上限，以及取消、退出和清理。复用 SSH 传输与现有对话框。
- #288：复用已有退出动作，明确 ⌘W 关闭窗口、⌘⇧W 关闭终端、⌘Q 退出；编辑菜单复用 GPUI 输入动作，About 入口选择应用设置首页。修复旧 keymap 与 Quit 默认绑定的冲突。
- #289：移除旧后端无效的动态 delegate 补丁，改用已锁定 GPUI 原生 UserNotifications 服务；保留真实运行模式的 legacy 回退，不初始化旧 NSBundle hook。所有原生入口复用同一服务，队列与回调保留上限均为 64，默认点击和有效选项仍走既有激活事件。产品提交 `0d454f441e3b26a128076383c6513be3b94d66c6` 的实际通知显示和点击激活验收已通过。
- #291：核对无 JavaScript 的移动导航、生成目录清理保护、实际安装包名及贡献指南。保留等待期间新增的补全、加密备份、Android 配对指南，并核对逐页来源版本与操作步骤。
- #294：保留发行渠道的更新限制；修复过期后台结果、来源重置与 About 链接。实际输入、键盘替换、校验、保存、重开和清空的回归与主线新主题编辑测试共同保留。

- #418：新 PR 在本轮被发现。复查发现隐藏驻留窗口会提前消耗一次性展示标记；已推送等待可见窗口再展示的修复，继续复用既有带锁的更新状态。新回归在独立进程测试真实隐藏/显示事件。第一次 CI 揭示新子模块对话框路径漏改，已修正并格式化；当前 head 正式 CI 的 10 项必需检查全部通过；Mac ARM 的实际 native suite 运行 2583 个测试全部通过，新隐藏窗口回归也实际通过（3.046 秒）。

17 条 Copilot 评论线程全部已解决，问题修复已对照代码核实。评论以具体问题和修改建议为主，属于建设性的审查；COMMENTED 不等于 APPROVED。

已完成正确性与最小性两轮独立复查，每次主线同步后重新核对最终差异。最终通知和更新说明的本地验证树与发布树一致；版本、翻译完整性与重复键检查通过，格式、空白和架构检查通过。没有修改合并规则或把 PR 专用验收工作流加入产品差异。#291 的最终文档构建与桌面/移动浏览器检查通过：[文档 CI](https://github.com/Kuddev/pebrel/actions/runs/36750468254)；本地 9 项文档测试通过。

## 最终提交的原生证据

[#128、#288 原生验收 run 36750717366](https://github.com/WilliamWang1721/pebrel/actions/runs/36750717366)均通过，结构化结果已下载复查：

- #128 `c99efb4`：真实点击三种启动选择、便携配置、驻留端点与 CLI、正常退出、移动应用和数据后重启。四个进程退出码均为 0，未出现 GPUI 初始化崩溃。
- #288 `571e0a3`：原生 New tab、创建窗口、⌘⇧W 关闭终端、⌘W 关闭窗口和 ⌘Q 正常退出。

#280 的 macOS 必需报告按既有路径策略汇报成功，没有实际执行 macOS 工作负载；本 PR 的实际原生作业覆盖 Linux、Windows x64 和 Windows ARM。不会把路径选择结果描述成五个平台都执行了测试。

#289 已在用户明确授权后执行公开截图验收，只上传合成测试界面的 PNG 和 acceptance.json，保留 7 天；不导出原始日志。

最终通过 run [36815676201](https://github.com/WilliamWang1721/pebrel/actions/runs/36815676201)，绑定产品提交 `0d454f441e3b26a128076383c6513be3b94d66c6`。实际注册 App 经 Launch Services 启动，前台 OSC 9 通过生产通知路径送达；通过系统设置的真实控件授权（状态 2），重启注册 App 后显示原生通知。验收单独截取系统通知进程所拥有的窗口，在其中识别正文，切换 Finder 后再次确认可见；真实 GUI session 鼠标点击后，Pebrel 成为前台应用且通知消失。成功 JSON 与 PNG 已下载复查。

测试桌面原先缺少通知显示 agent，通过当前临时 GUI 用户会话的正式 launchctl 接口启动系统自带服务；未编辑受保护配置或权限数据库。此前失败包括旧后端不可见、辅助功能树未暴露原生文字、窄窗口筛选漏掉透明全屏窗口，以及 AX 点击没有实际送达。相关失败没有被标记为成功。最终验收采用拥有者窗口截图和真实鼠标事件，未修改产品以迎合验收，也未绕过通知授权。

## 剩余合并门槛

1. main 要求 Kuddev 的一次新 code-owner 批准；作者不能代替，提交更新会撤销旧审批。目前原六个 PR 和新增 #418 均没有有效新批准，#280 的前次批准已被撤销。
2. #288 仍是 Draft；本轮再次尝试转换 Ready 返回 FORBIDDEN / Resource not accessible by integration。当前 GitHub 连接修改上游 PR、发布验证评论和转换 Ready 均返回 403/FORBIDDEN；因此上游 PR 描述仍含较早提交的验证信息。
3. 等待当前七个提交的所有必需检查；#289 最终提交的原生界面与点击验收已通过。

用户已于本轮明确同意 PNG 与合成 JSON 的公开上传。原始日志不在授权范围内，未重新导出。

本轮由 Codex（GPT-6）协助完成；具体模型 variant 和 reasoning effort 未在当前运行环境提供。
