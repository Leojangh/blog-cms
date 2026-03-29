# Odd Studio — Blog CMS

前后端分离的博客系统。后端 Rust (Axum) 提供 RESTful JSON API，前端 Astro 静态生成。

## 架构

```
genlz.com (Cloudflare Pages)  ──  静态前端 (Astro SSG)
api.genlz.com (Cloudflare Tunnel)  ──  Rust API (Axum + SQLite)
```

- 博客前台：Astro 构建时从 API 拉取数据生成静态 HTML，部署到 Cloudflare Pages CDN
- 管理后台：纯客户端渲染 (CSR)，通过 fetch 调用 API
- 后端 API：Rust + Axum，SQLite 存储，HMAC 签名 session，Argon2 密码哈希

## 项目结构

```
blog-cms/
├── src/                    # Rust 后端
│   ├── main.rs             # 路由定义
│   ├── handlers/
│   │   ├── blog.rs         # 公开 API: GET /api/posts, GET /api/posts/:slug
│   │   └── admin.rs        # 管理 API: CRUD、登录、图片上传
│   ├── middleware/mod.rs    # 认证、CORS、安全头
│   └── models/mod.rs       # SQLite 初始化
├── frontend/               # Astro 前端
│   ├── src/pages/          # 页面
│   │   ├── index.astro     # 首页 (SSG)
│   │   ├── post/[slug].astro  # 文章页 (SSG)
│   │   └── admin/          # 管理后台 (CSR)
│   └── astro.config.mjs
├── deploy.sh               # Linux 后端部署脚本
├── deploy-android.sh       # Android 后端部署脚本
├── blog-cms-android.sh     # Android 服务管理脚本（部署后推送到设备）
└── Cargo.toml
```

## API 端点

| 方法 | 路径 | 说明 | 认证 |
|------|------|------|------|
| GET | `/api/posts` | 文章列表 | 否 |
| GET | `/api/posts/:slug` | 文章详情（自动 +1 浏览量） | 否 |
| POST | `/api/auth/login` | 登录 | 否 |
| POST | `/api/auth/logout` | 登出 | 否 |
| GET | `/api/auth/me` | 检查登录状态 | 否 |
| GET | `/api/admin/dashboard` | 仪表盘（文章列表 + 浏览量统计） | 是 |
| POST | `/api/admin/posts` | 创建文章 | 是 |
| GET | `/api/admin/posts/:slug` | 获取文章（编辑用） | 是 |
| PUT | `/api/admin/posts/:slug` | 更新文章 | 是 |
| DELETE | `/api/admin/posts/:slug` | 删除文章（同时清理关联图片） | 是 |
| POST | `/api/admin/upload` | 上传图片（≤5MB） | 是 |
| POST | `/api/admin/delete-image` | 删除图片 | 是 |

## 环境变量

后端 `.env`（位于 `/opt/blog-cms/.env`）：

```
SESSION_SECRET=<随机 hex 字符串>
ADMIN_USER=admin
ADMIN_PASS_HASH=<argon2id 哈希>
DATABASE_PATH=/opt/blog-cms/blog.db
CORS_ORIGIN=https://genlz.com
```

前端构建时环境变量：

```
PUBLIC_API_URL=https://api.genlz.com
```

## 本地开发

### 后端

```bash
# 需要 Rust toolchain
cargo run
# 默认监听 http://127.0.0.1:3000
```

### 前端

```bash
cd frontend
bun install
PUBLIC_API_URL=http://127.0.0.1:3000 bun run dev
```

## 编译与部署

### 前置条件

- Rust toolchain：`rustup`
- Bun：`brew install oven-sh/bun/bun`

#### Linux 服务器部署额外依赖

- `cargo-zigbuild`：`cargo install cargo-zigbuild`
- Zig：`brew install zig`
- musl target：`rustup target add x86_64-unknown-linux-musl`

#### Android 设备部署额外依赖

- Android NDK（通过 Android Studio SDK Manager 安装）
- Android target：`rustup target add aarch64-linux-android`
- ADB 连接到目标设备

### 部署后端（Linux 服务器）

```bash
./deploy.sh
```

脚本执行：
1. `cargo zigbuild --release --target x86_64-unknown-linux-musl`
2. SSH 停止远程 blog-cms 服务
3. SCP 上传二进制到 `/opt/blog-cms/`
4. SSH 启动服务

### 部署后端（Android 设备）

部署目录：`/mnt/scratch/overlay/system_ext/upper/blog-cms`

```bash
./deploy-android.sh
```

脚本执行：
1. 使用 NDK toolchain 交叉编译 `aarch64-linux-android` target
2. 通过 ADB 推送二进制、`.env`（自动替换 `DATABASE_PATH`）和管理脚本到设备
3. 设备上通过管理脚本控制服务

#### 管理脚本

设备上的 `blog-cms.sh`（可通过软链接 `/system_ext/bin/blog-cms` 调用）：

```bash
blog-cms start     # 启动服务
blog-cms stop      # 停止服务
blog-cms restart   # 重启服务
blog-cms status    # 查看运行状态
blog-cms watch     # 守护模式（进程退出后每 5 秒自动重启）
blog-cms log       # 查看日志
```

#### Cloudflare Tunnel（Android）

设备需安装 `cloudflared`，配置文件位于 `<部署目录>/.cloudflared/`：

```bash
# 登录（需要浏览器授权）
HOME=<部署目录> SSL_CERT_DIR=/system/etc/security/cacerts cloudflared tunnel login

# 创建 tunnel
HOME=<部署目录> SSL_CERT_DIR=/system/etc/security/cacerts cloudflared tunnel create blog-cms

# 配置 DNS
HOME=<部署目录> SSL_CERT_DIR=/system/etc/security/cacerts cloudflared tunnel route dns blog-cms api.genlz.com
```

`config.yml`：

```yaml
tunnel: <tunnel-id>
credentials-file: <部署目录>/.cloudflared/<tunnel-id>.json

ingress:
  - hostname: api.genlz.com
    service: http://127.0.0.1:3000
  - service: http_status:404
```

启动 tunnel：

```bash
HOME=<部署目录> SSL_CERT_DIR=/system/etc/security/cacerts \
  cloudflared tunnel --config <部署目录>/.cloudflared/config.yml run
```

> **注意**：Android 上需设置 `SSL_CERT_DIR=/system/etc/security/cacerts` 以解决 TLS 证书验证问题。

#### 开机自启（Android init.rc）

在 `/system_ext/etc/init/` 下创建 `blog-cms.rc`，通过 Android init 系统实现开机自启：

```ini
service blog-cms /system_ext/blog-cms/blog-cms.sh watch
    class late_start
    user root
    group root inet
    seclabel u:r:su:s0
    setenv HOME <部署目录>
    setenv SSL_CERT_DIR /system/etc/security/cacerts

service cloudflared /system_ext/bin/cloudflared tunnel --config /system_ext/blog-cms/.cloudflared/config.yml run
    class late_start
    user root
    group root inet
    seclabel u:r:su:s0
    setenv HOME <部署目录>
    setenv SSL_CERT_DIR /system/etc/security/cacerts
```

写入方式（需通过 overlay upper 目录）：

```bash
# 将文件写入 overlay upper（需 adb root）
adb root
cp /data/local/tmp/blog-cms.rc /mnt/scratch/overlay/system_ext/upper/etc/init/blog-cms.rc
chmod 644 /mnt/scratch/overlay/system_ext/upper/etc/init/blog-cms.rc
chcon u:object_r:system_file:s0 /mnt/scratch/overlay/system_ext/upper/etc/init/blog-cms.rc
```

> **注意事项**：
> - `.rc` 文件权限必须为 `644`，否则 init 会以 `insecure file` 拒绝解析
> - SELinux 上下文必须为 `u:object_r:system_file:s0`
> - 通过 `adb push` 推送的二进制文件可能被标记为 `overlayfs_file`，需用 `chcon u:object_r:system_file:s0` 修正
> - 不要使用 `oneshot`，否则主进程退出后 init 会 SIGKILL 整个进程组
> - `class late_start` 确保网络就绪后再启动服务

### 部署前端

```bash
cd frontend
PUBLIC_API_URL=https://api.genlz.com bun run build
# 将 dist/ 目录上传到 Cloudflare Pages (Direct Upload)
```

## 服务器配置（Linux）

### systemd 服务

`/etc/systemd/system/blog-cms.service`：

```ini
[Unit]
Description=Blog CMS
After=network.target

[Service]
Type=simple
WorkingDirectory=/opt/blog-cms
ExecStart=/opt/blog-cms/blog-cms
EnvironmentFile=/opt/blog-cms/.env
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### Cloudflare Tunnel（Linux）

`/etc/cloudflared/config.yml`：

```yaml
tunnel: <tunnel-id>
credentials-file: /root/.cloudflared/<tunnel-id>.json

ingress:
  - hostname: api.genlz.com
    service: http://127.0.0.1:3000
  - service: http_status:404
```

### 生成管理员密码哈希

```bash
python3 -c "from argon2 import PasswordHasher; print(PasswordHasher().hash('你的密码'))"
```

## 安全特性

- Argon2id 密码哈希
- HMAC-SHA256 session 签名 + 过期时间
- 常量时间比较（防时序攻击）
- Cookie: HttpOnly / Secure / SameSite
- 登录限流（递增冷却）
- CORS 白名单
- 安全响应头（X-Content-Type-Options, X-Frame-Options, Referrer-Policy）
- Slug 格式校验
- 图片上传格式和大小限制
- 服务仅监听 127.0.0.1，通过 Cloudflare Tunnel 暴露
