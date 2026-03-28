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
├── deploy.sh               # 后端部署脚本
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

- Rust + `cargo-zigbuild`（交叉编译）：`cargo install cargo-zigbuild`
- Zig：`brew install zig`
- Bun：`brew install oven-sh/bun/bun`
- musl target：`rustup target add x86_64-unknown-linux-musl`

### 部署后端

```bash
./deploy.sh
```

脚本执行：
1. `cargo zigbuild --release --target x86_64-unknown-linux-musl`
2. SSH 停止远程 blog-cms 服务
3. SCP 上传二进制和静态文件到 `/opt/blog-cms/`
4. SSH 启动服务

### 部署前端

```bash
cd frontend
PUBLIC_API_URL=https://api.genlz.com bun run build
# 将 dist/ 目录上传到 Cloudflare Pages (Direct Upload)
```

## 服务器配置

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

### Cloudflare Tunnel

`/etc/cloudflared/config.yml`：

```yaml
tunnel: <tunnel-id>
credentials-file: /root/.cloudflared/<tunnel-id>.json

ingress:
  - hostname: api.genlz.com
    service: http://127.0.0.1:3000
  - hostname: genlz.com
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
