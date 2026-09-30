# Pebrel 六个 PR 的合并准备报告

状态：修复已推送，最终 CI 仍在完成，尚未达到 100% Ready to Merge。记录时间：2026-09-30 16:06 UTC。

目标 main：`5ff2f3eea54b9c66e416149e17e823b8d72b1dee`。六个 PR 均已同步到该版本。没有合并 PR，也没有 force push。

| PR | 最终提交 | 必需检查 | 正式检查记录 |
| --- | --- | --- | --- |
| [#128](https://github.com/Kuddev/pebrel/pull/128) macOS 便携启动 | `cf55c0d16db6904b9eda3b05fbcc7afa6531cb0b` | 10/10 全部通过 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36738649120) |
| [#280](https://github.com/Kuddev/pebrel/pull/280) SSH 本地端口转发 | `457d4a95e2247ade8f286742c58b3e7860121ebc` | 10/10 全部通过 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36738680063) |
| [#288](https://github.com/Kuddev/pebrel/pull/288) macOS 原生菜单 | `fc2529b125f3c890a54d997503e87c5da73ad101` | 3/10；其余仍运行或排队 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36738717424) |
| [#289](https://github.com/Kuddev/pebrel/pull/289) macOS 前台系统通知 | `03d992b96dc3239fee42b2a9ef059135c8f1bfd6` | 3/10；其余仍运行或排队 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36738803423) |
| [#291](https://github.com/Kuddev/pebrel/pull/291) 用户文档站点 | `1f42b3f57341fff6874669fce04f81de802ee6b5` | 3/10；其余仍运行或排队 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36738836135) |
| [#294](https://github.com/Kuddev/pebrel/pull/294) 自定义更新来源 | `45e8ce4bb94c8ef060f561f018d3ba8e3619da9e` | 3/10；其余仍运行或排队 | [正式 CI](https://github.com/Kuddev/pebrel/actions/runs/36738871406) |

## 修复与复查

- #128：保留最新 main 的启动行为，修复 daemon/转位路径判断，并修复真实点击便携启动后出现的 GPUI 初始化崩溃。使用已锁定的 Core Foundation 绑定直接调用原生提示，避免 RFD 提前初始化普通 NSApplication。
- #280：解决 SSH 模块合并冲突，保留当前 transcript 模块，核对 listener 重试、真实拒绝协议、并发上限、取消和清理。传输、生命周期与实际渲染的对话框回归通过。
- #288：复用已有安全退出动作；⌘W 关闭窗口，⌘⇧W 关闭终端，⌘Q 退出。About 入口选择应用设置首页。
- #289：保留现有系统通知派发器，补充 macOS 前台呈现回调；系统实际显示的验收仍未通过。
- #291：核对无 JavaScript 移动导航、生成目录清理与保护、实际安装包名、贡献指南与当前 CI 策略。
- #294：保留当前发行渠道的更新限制，修复过期后台结果、重置来源与链接；新增实际输入、键盘、保存、重开和清空的回归。

17 条 Copilot 评论线程均已解决，且问题修复已对照代码核实。评论以具体问题和修改建议为主，属于建设性的代码审查。COMMENTED 不代表 APPROVED。

完成了正确性与最小性两轮复查，并在每次 main 同步后再次核对最终差异。最后一次同步仅加入当前 main 的移动端改动，各 PR 的功能文件与前一轮完全一致。本地格式、空白、架构与 cfg 预算检查通过；87 项 CI/架构 Python 检查和 5 项 PR 大小检查通过。最终正式 CI 结果以表中 head 的记录为准。

## 原生交互证据

[#128、#288 最终提交的原生验收](https://github.com/WilliamWang1721/pebrel/actions/runs/36738955428)：两个作业均通过。

- #128：真实点击三种启动选择，检查便携配置、CLI 连接、正常退出、移动应用及数据后的重启。
- #288：真实菜单点击、创建终端和窗口，以及 ⌘⇧W、⌘W、⌘Q 的对应关闭/退出行为。
- #280 的 macOS 必需报告按照路径选择通过，没有实际执行 macOS 工作负载。其他 PR 的 macOS 工作负载按正式 CI 策略选择；未执行的工作负载没有被描述为执行通过。

[#289 最近的系统显示验收](https://github.com/WilliamWang1721/pebrel/actions/runs/36736719358)未通过：同一签名测试 App 的通知授权申请超时，未检测到对应授权提示，随后状态为拒绝。发送、注册、前台回调安装和激活没有报告错误，但这不能证明通知实际可见。该验收基于 `665402b56d7021dc0b63381712291b62aa8c656c`；当前 head 合入的是移动端改动，桌面通知实现保持一致。此记录仍是失败的验收证据。

## 剩余门槛

1. 完成所有最终 head 的必需 CI；此前 head 的绿灯不能替代。
2. main 要求 Kuddev 的一次新 code-owner 审批；作者不能代替。更新会撤销旧审批，#280 的前次审批已被撤销。
3. #288 仍为 Draft。当前 GitHub 集成修改上游 PR、发布验证评论和转换 Ready 均返回 403/FORBIDDEN，因此上游描述仍保留较早提交的验证说明。
4. #289 系统通知实际显示的验收仍需完成。

自动审批曾拒绝重新打包原生日志和截图，理由是它们可能包含 runner 路径、命令及环境信息。已请求用户明确授权，尚未收到答复。被拒绝的产物没有重新导出；后续仅导出合成检查名称、布尔值和数值状态。

本轮由 Codex（GPT-6）协助完成；具体模型 variant 与 reasoning effort 未在当前运行环境提供。
