# MMC (Multi-Device Communication)

跨平台多设备通信框架，支持设备发现、配对、文件传输、剪贴板同步、屏幕镜像和远程控制。

## 目录

- [项目概述](#项目概述)
- [系统架构](#系统架构)
- [环境要求](#环境要求)
- [快速开始](#快速开始)
- [桌面应用使用教程](#桌面应用使用教程)
- [Android 开发指南](#android-开发指南)
- [iOS 开发指南](#ios-开发指南)
- [API 参考](#api-参考)
- [构建与发布](#构建与发布)
- [常见问题](#常见问题)

---

## 项目概述

MMC 是一个用 Rust 编写的跨平台通信框架，旨在实现移动设备与桌面设备之间的无缝协作。主要特性包括：

| 功能 | 描述 |
|------|------|
| 设备发现 | 通过 mDNS/DNS-SD 自动发现局域网内的 MMC 设备 |
| 安全配对 | TLS 1.3 + ECDH 密钥交换，确保通信安全 |
| 文件传输 | 分块传输、校验和验证、断点续传支持 |
| 剪贴板同步 | 跨设备实时同步文本、图片、URL 内容 |
| 屏幕镜像 | 实时屏幕捕获与显示 (Windows/Android) |
| 远程控制 | 触摸事件、键盘事件注入 |

---

## 系统架构

```
mmc-core/
├── crates/
│   ├── mmc-protocol      # TCP帧协议、JSON/Protobuf序列化
│   ├── mmc-security      # TLS 1.3、证书管理、加密
│   ├── mmc-transport     # TCP连接管理、心跳保活
│   ├── mmc-discovery     # mDNS设备发现
│   ├── mmc-pairing       # 设备配对认证
│   ├── mmc-file-transfer # 文件传输服务
│   ├── mmc-clipboard     # 剪贴板同步
│   ├── mmc-media-service # 屏幕/音频/输入处理
│   ├── mmc-storage       # SQLite持久化存储
│   ├── mmc-core-uniffi   # 统一API + UniFFI绑定
│   └── mmc-desktop-app   # 桌面CLI应用
├── proto/                # Protobuf定义文件
└── bindings/             # Android/iOS绑定代码
```

---

## 环境要求

### 通用要求

| 组件 | 版本要求 |
|------|----------|
| Rust | 1.75+ |
| Cargo | 最新稳定版 |

### 平台特定要求

#### Windows
- Visual Studio Build Tools (MSVC) 或 MinGW-w64 (GNU)
- Windows 10/11

#### Android
- Android NDK r24+
- cargo-ndk: `cargo install cargo-ndk`
- Rust Android targets:
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  ```

#### iOS
- Xcode 14+
- iOS 15+ 设备

---

## 快速开始

### 1. 克隆项目

```bash
git clone https://github.com/your-org/mmc.git
cd mmc/mmc-core
```

### 2. 构建项目

```bash
# 构建所有模块
cargo build --release

# 仅构建桌面应用
cargo build --release --bin mmc-desktop
```

### 3. 运行测试

```bash
cargo test --all
```

### 4. 运行桌面应用

```bash
# Windows
.\target\release\mmc-desktop.exe info

# Linux/macOS
./target/release/mmc-desktop info
```

---

## 桌面应用使用教程

### 命令行界面

MMC 桌面应用提供完整的 CLI 界面：

```bash
mmc-desktop [COMMAND] [OPTIONS]
```

### 可用命令

#### 设备管理

```bash
# 发现局域网内的 MMC 设备
mmc-desktop device discover

# 列出已配对的设备
mmc-desktop device list

# 与指定设备配对
mmc-desktop device pair --device-id <DEVICE_ID>

# 解除配对
mmc-desktop device unpair --device-id <DEVICE_ID>
```

#### 文件传输

```bash
# 发送文件到指定设备
mmc-desktop transfer send --device-id <DEVICE_ID> --file <FILE_PATH>

# 查看传输任务列表
mmc-desktop transfer list

# 取消传输任务
mmc-desktop transfer cancel --task-id <TASK_ID>
```

#### 剪贴板同步

```bash
# 获取当前剪贴板内容
mmc-desktop clipboard get

# 设置剪贴板内容
mmc-desktop clipboard set --text "Hello World"

# 与指定设备同步剪贴板
mmc-desktop clipboard sync --device-id <DEVICE_ID>

# 监控剪贴板变化 (持续60秒)
mmc-desktop clipboard monitor --duration-secs 60
```

#### 屏幕镜像

```bash
# 开始屏幕镜像
mmc-desktop mirror start --device-id <DEVICE_ID>

# 查看镜像状态
mmc-desktop mirror status

# 停止屏幕镜像
mmc-desktop mirror stop
```

#### 交互模式

```bash
# 进入交互式 REPL 模式
mmc-desktop interactive
```

交互模式可用命令：

| 命令 | 说明 |
|------|------|
| `help` | 显示帮助信息 |
| `discover` | 发现附近设备 |
| `pair <device_id>` | 配对设备 |
| `devices` | 列出已配对设备 |
| `unpair <device_id>` | 解除配对 |
| `send <device_id> <file>` | 发送文件 |
| `transfers` | 查看传输任务 |
| `clipboard get` | 获取剪贴板 |
| `clipboard set <text>` | 设置剪贴板 |
| `mirror <device_id>` | 开始镜像 |
| `mirror stop` | 停止镜像 |
| `info` | 显示设备信息 |
| `quit` | 退出交互模式 |

#### 系统信息

```bash
# 显示平台和设备信息
mmc-desktop info
```

输出示例：
```
MMC Desktop Application
Platform: Windows
Device ID: abc123-def456
Device Name: My-PC
Version: 0.1.0
```

---

## Android 开发指南

### 1. 设置环境变量

```bash
# 设置 Android NDK 路径
export ANDROID_NDK_HOME=/path/to/android-ndk
# 或
export ANDROID_NDK_ROOT=/path/to/android-ndk
```

### 2. 安装 Rust Android 目标

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
```

### 3. 安装 cargo-ndk

```bash
cargo install cargo-ndk
```

### 4. 构建原生库

```bash
cd mmc-core

# 使用 Makefile 构建所有 ABI
make build-all

# 或手动构建单个 ABI
cargo ndk --target arm64-v8a build --release -p mmc-core-uniffi
```

### 5. 复制库文件到 Android 项目

```bash
make copy-libs
```

库文件将复制到：
```
bindings/android/app/src/main/jniLibs/
├── arm64-v8a/libmmc_core.so
├── armeabi-v7a/libmmc_core.so
└── x86_64/libmmc_core.so
```

### 6. 在 Android 项目中使用

#### Kotlin 示例

```kotlin
import com.example.mmc.MmcCore

class MainActivity : AppCompatActivity() {
    private lateinit var mmcCore: MmcCore

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // 初始化 MMC Core
        mmcCore = MmcCore()
        val config = CoreConfig(
            deviceId = "android-${Build.DEVICE}",
            deviceName = "${Build.MODEL}",
            deviceType = DeviceType.PHONE,
            appVersion = "1.0.0"
        )
        mmcCore.init(config)

        // 发现设备
        mmcCore.discoverDevices { devices ->
            devices.forEach { device ->
                Log.d("MMC", "Found: ${device.name} at ${device.ip}")
            }
        }

        // 发送文件
        mmcCore.sendFile(deviceId, filePath) { progress ->
            Log.d("MMC", "Progress: ${progress.percent}%")
        }
    }
}
```

---

## iOS 开发指南

### 1. 构建 iOS 框架

```bash
# 添加 iOS 目标
rustup target add aarch64-apple-ios x86_64-apple-ios

# 构建
cargo build --release --target aarch64-apple-ios -p mmc-core-uniffi
```

### 2. 创建 XCFramework

```bash
# 生成 UniFFI Swift 绑定
cargo run --bin uniffi-bindgen -- generate src/mmc_core.udl --language swift

# 创建 XCFramework (需要手动整合)
```

### 3. Swift 使用示例

```swift
import MmcCore

class ViewController: UIViewController {
    var mmcCore: MmcCore?

    override func viewDidLoad() {
        super.viewDidLoad()

        // 初始化
        mmcCore = MmcCore()
        let config = CoreConfig(
            deviceId: UIDevice.current.identifierForVendor?.uuidString ?? "unknown",
            deviceName: UIDevice.current.name,
            deviceType: DeviceType.phone,
            appVersion: "1.0.0"
        )
        mmcCore?.init(config)

        // 发现设备
        mmcCore?.discoverDevices { devices in
            for device in devices {
                print("Found: \(device.name)")
            }
        }
    }

    // 发送文件
    func sendFile(deviceId: String, fileURL: URL) {
        mmcCore?.sendFile(deviceId, fileURL.path) { progress in
            print("Progress: \(progress.percent)%")
        }
    }
}
```

---

## API 参考

### 核心类型

#### DeviceType

```rust
pub enum DeviceType {
    Phone,
    Tablet,
    Desktop,
    Laptop,
    Tv,
    Other,
}
```

#### DeviceInfo

```rust
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub ip: String,
    pub port: u16,
    pub capabilities: Capabilities,
}
```

#### Capabilities

```rust
pub struct Capabilities {
    pub file_transfer: bool,
    pub screen_mirror: bool,
    pub remote_control: bool,
    pub clipboard_sync: bool,
}
```

### 主要方法

#### MmcCore

| 方法 | 说明 |
|------|------|
| `init(config: CoreConfig)` | 初始化核心库 |
| `discoverDevices()` | 发现附近设备 |
| `pairDevice(deviceId: String)` | 配对设备 |
| `unpairDevice(deviceId: String)` | 解除配对 |
| `getPairedDevices()` | 获取已配对设备列表 |
| `sendFile(deviceId: String, path: String)` | 发送文件 |
| `cancelTransfer(taskId: String)` | 取消传输 |
| `getClipboardContent()` | 获取剪贴板内容 |
| `setClipboardContent(text: String)` | 设置剪贴板 |
| `startMirror(deviceId: String)` | 开始屏幕镜像 |
| `stopMirror()` | 停止屏幕镜像 |

---

## 构建与发布

### 本地构建

```bash
# Debug 构建
cargo build

# Release 构建 (优化性能)
cargo build --release

# 运行所有测试
cargo test --all

# 代码质量检查
cargo clippy --all-targets --all-features
```

### GitHub Actions 自动构建

项目配置了 GitHub Actions CI/CD，自动构建 Windows 可执行文件：

#### 触发条件

- 推送到 `main` 或 `master` 分支
- 推送版本标签 (如 `v1.0.0`)
- 手动触发 (`workflow_dispatch`)

#### 构建产物

| 目标平台 | 文件名 |
|----------|--------|
| Windows x64 MSVC | `mmc-desktop-x64-msvc.zip` |
| Windows x64 GNU | `mmc-desktop-x64-gnu.zip` |

#### 下载构建产物

1. 进入 GitHub 仓库页面
2. 点击 **Actions** 标签
3. 选择最新的构建工作流
4. 在 **Artifacts** 区域下载 ZIP 文件

#### 发布新版本

```bash
# 创建版本标签
git tag v1.0.0
git push origin v1.0.0

# GitHub Actions 将自动构建并发布到 Releases
```

---

## 常见问题

### Q: Windows 构建失败，提示找不到链接器？

**A:** 安装 Visual Studio Build Tools：
1. 下载 [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)
2. 选择 "C++ build tools" 工作负载
3. 重启终端后重新构建

### Q: Android 构建失败，找不到 NDK？

**A:** 设置环境变量：
```bash
export ANDROID_NDK_HOME=/path/to/android-ndk-r24
```

### Q: 剪贴板功能在 Linux 上不工作？

**A:** 确保安装了 X11 或 Wayland 相关依赖：
```bash
# Ubuntu/Debian
sudo apt install libxcb1-dev

# Fedora
sudo dnf install libxcb-devel
```

### Q: 设备发现不工作？

**A:** 检查网络配置：
1. 确保设备在同一局域网
2. 检查防火墙是否阻止 mDNS (端口 5353 UDP)
3. 确保路由器支持 mDNS/Bonjour

### Q: 如何查看详细日志？

**A:** 设置日志级别：
```bash
# 设置环境变量
export RUST_LOG=debug

# 或在代码中配置
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

---

## 许可证

本项目采用双重许可：

- MIT License
- Apache License 2.0

可根据需要选择任一许可证。

---

## 贡献指南

欢迎提交 Issue 和 Pull Request！

1. Fork 本仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

---

## 联系方式

- GitHub Issues: [项目 Issues 页面](https://github.com/your-org/mmc/issues)
- Email: mmc-team@example.com