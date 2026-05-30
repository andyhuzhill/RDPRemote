# RDPRemote 部署指南

> 生产环境部署手册

## 目录

- [部署选项](#部署选项)
- [Docker 部署](#docker-部署)
- [Kubernetes 部署](#kubernetes-部署)
- [裸机部署](#裸机部署)
- [网络配置](#网络配置)
- [监控与日志](#监控与日志)
- [故障排查](#故障排查)

---

## 部署选项

| 方案 | 适用场景 | 复杂度 |
|------|---------|--------|
| Docker Compose | 单机部署、开发测试 | ⭐ |
| Kubernetes | 生产集群、高可用 | ⭐⭐⭐ |
| 裸机二进制 | 极简环境、边缘部署 | ⭐⭐ |

---

## Docker 部署

### 前置条件

- Docker 20.10+
- Docker Compose v2.0+

### 快速部署

```bash
# 1. 克隆仓库
git clone https://github.com/your-org/RDPRemote.git
cd RDPRemote

# 2. 构建镜像
docker-compose build

# 3. 启动服务
docker-compose up -d

# 4. 验证运行
docker-compose ps
```

### docker-compose.yml 详解

```yaml
# 生产环境配置示例
version: "3.9"

services:
  rdp-server:
    build:
      context: .
      dockerfile: Dockerfile
      target: runtime
    image: rdpremove-server:v1.0.0
    container_name: rdpremove-signaling
    restart: unless-stopped
    ports:
      - "8765:8765"
    volumes:
      - ./data/server:/app/data
    environment:
      - RUST_LOG=info
      - SIGNALING_PORT=8765
      - MAX_PEERS=100
      - SESSION_TIMEOUT=3600
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:8765/"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10s
    networks:
      - rdpremove-network
    deploy:
      resources:
        limits:
          cpus: '1'
          memory: 512M
        reservations:
          cpus: '0.25'
          memory: 128M

  # TURN 服务器（可选，用于 NAT 穿越）
  coturn:
    image: coturn/coturn:latest
    container_name: rdpremove-turn
    restart: unless-stopped
    ports:
      - "3478:3478/tcp"
      - "3478:3478/udp"
      - "5349:5349/tcp"
      - "5349:5349/udp"
      - "10000-20000:10000-20000/udp"  # UDP 端口范围
    volumes:
      - ./turn/turnserver.conf:/etc/turnserver.conf:ro
    command:
      - "-c /etc/turnserver.conf"
    networks:
      - rdpremove-network

networks:
  rdpremove-network:
    driver: bridge

volumes:
  data:
```

### 多节点部署

```bash
# 使用 Swarm 模式部署
docker swarm init

# 部署服务
docker stack deploy -c docker-compose.swarm.yml rdpremove

# 查看服务状态
docker stack ps rdpremove
```

---

## Kubernetes 部署

### 部署清单

```yaml
# k8s/signaling-server.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rdpremove-signaling
  labels:
    app: rdpremove
    component: signaling
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rdpremove
      component: signaling
  template:
    metadata:
      labels:
        app: rdpremove
        component: signaling
    spec:
      containers:
      - name: rdp-server
        image: your-registry/rdpremove-server:v1.0.0
        ports:
        - containerPort: 8765
          name: signaling
          protocol: TCP
        env:
        - name: RUST_LOG
          value: "info"
        - name: SIGNALING_PORT
          value: "8765"
        resources:
          requests:
            cpu: 100m
            memory: 128Mi
          limits:
            cpu: 500m
            memory: 512Mi
        livenessProbe:
          httpGet:
            path: /
            port: 8765
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /
            port: 8765
          initialDelaySeconds: 5
          periodSeconds: 10
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchLabels:
                  app: rdpremove
                  component: signaling
              topologyKey: kubernetes.io/hostname
---
apiVersion: v1
kind: Service
metadata:
  name: rdpremove-signaling
spec:
  selector:
    app: rdpremove
    component: signaling
  ports:
  - port: 8765
    targetPort: 8765
    name: signaling
  type: LoadBalancer
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: rdpremove-ingress
  annotations:
    nginx.ingress.kubernetes.io/backend-protocol: "WS"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "3600"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "3600"
spec:
  rules:
  - host: rdpremove.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: rdpremove-signaling
            port:
              number: 8765
```

### 部署命令

```bash
# 1. 创建命名空间
kubectl create namespace rdpremove

# 2. 应用配置
kubectl apply -f k8s/ -n rdpremove

# 3. 验证部署
kubectl get pods -n rdpremove -l app=rdpremove

# 4. 查看日志
kubectl logs -f deployment/rdpremove-signaling -n rdpremove
```

---

## 裸机部署

### 编译 Release 版本

```bash
# Linux
cargo build --release -p rdp-server

# Windows (cross-compile from Linux)
cargo build --release -p rdp-agent --target x86_64-pc-windows-msvc

# macOS
cargo build --release -p rdp-client
```

### systemd 服务配置

```ini
# /etc/systemd/system/rdpremove-server.service
[Unit]
Description=RDPRemote Signaling Server
After=network.target

[Service]
Type=simple
User=rdpremove
Group=rdpremove
WorkingDirectory=/opt/rdpremove
ExecStart=/opt/rdpremove/rdp-server
Restart=always
RestartSec=5
Environment=RUST_LOG=info
Environment=SIGNALING_PORT=8765

# 安全加固
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/rdpremove/data

[Install]
WantedBy=multi-user.target
```

### 启动服务

```bash
# 创建用户
sudo useradd -r -s /bin/false rdpremove

# 安装二进制
sudo mkdir -p /opt/rdpremove/data
sudo cp target/release/rdp-server /opt/rdpremove/
sudo chown -R rdpremove:rdpremove /opt/rdpremove

# 启用服务
sudo systemctl daemon-reload
sudo systemctl enable rdpremove-server
sudo systemctl start rdpremove-server

# 查看状态
sudo systemctl status rdpremove-server
```

---

## 网络配置

### 防火墙规则

```bash
# iptables
iptables -A INPUT -p tcp --dport 8765 -j ACCEPT
iptables -A INPUT -p udp --dport 3478:5349 -j ACCEPT
iptables -A INPUT -p udp --dport 10000:20000 -j ACCEPT

# firewalld (CentOS/RHEL)
firewall-cmd --permanent --add-port=8765/tcp
firewall-cmd --permanent --add-port=3478-5349/tcp
firewall-cmd --permanent --add-port=3478-5349/udp
firewall-cmd --permanent --add-port=10000-20000/udp
firewall-cmd --reload

# ufw (Ubuntu)
ufw allow 8765/tcp
ufw allow 3478:5349/tcp
ufw allow 3478:5349/udp
ufw allow 10000:20000/udp
```

### TURN 服务器配置

```conf
# /etc/turnserver.conf
listening-port=3478
tls-listening-port=5349
listening-ip=0.0.0.0
external-ip=YOUR_PUBLIC_IP

# 认证
user=rdpremove:YOUR_SECRET_PASSWORD
realm=rdpremove.example.com
static-auth-secret=YOUR_SECRET

# 端口范围
min-port=10000
max-port=20000

# 日志
log-file=/var/log/turnserver.log
syslog
```

---

## 监控与日志

### Prometheus 监控

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'rdpremove'
    static_configs:
      - targets: ['rdpremove-signaling:8765']
    metrics_path: '/metrics'
```

### 日志收集

```bash
# Docker 日志
docker-compose logs -f rdp-server

# 日志轮转
docker-compose up -d --log-opt max-size=10m --log-opt max-file=3

# Fluentd 收集
# 配置 Fluentd 将日志发送到 Elasticsearch
```

### 健康检查端点

| 端点 | 描述 |
|------|------|
| `GET /` | 服务健康检查 |
| `GET /metrics` | Prometheus 指标 |
| `GET /health` | 详细健康状态 |

---

## 故障排查

### 常见问题

#### 1. 客户端无法连接服务器

```bash
# 检查服务状态
docker-compose ps
systemctl status rdpremove-server

# 检查端口监听
netstat -tlnp | grep 8765

# 测试连接
curl -v ws://localhost:8765/
```

#### 2. WebRTC 连接失败

```bash
# 检查 TURN 服务器
turnutils_uclient -n YOUR_PUBLIC_IP

# 检查 ICE 候选
# 在浏览器控制台查看 WebRTC 日志

# 检查防火墙
# 确保 UDP 端口范围开放
```

#### 3. 内存占用过高

```bash
# 查看内存使用
docker stats rdpremove-signaling

# 调整资源限制
# 在 docker-compose.yml 中调整 deploy.resources.limits

# 检查连接数
ss -s | grep tcp
```

### 调试模式

```bash
# 启用详细日志
export RUST_LOG=debug
docker-compose up -d

# 查看实时日志
docker-compose logs -f --tail=100
```

---

## 升级指南

### Docker 升级

```bash
# 1. 拉取新镜像
docker-compose pull

# 2. 备份数据
docker run --rm -v rdpremove-data:/data -v $(pwd):/backup alpine tar czf /backup/data-backup.tar.gz /data

# 3. 滚动更新
docker-compose up -d

# 4. 验证
docker-compose ps
```

### 回滚

```bash
# 使用旧版本镜像
docker-compose pull rdpremove-server:previous-version
docker-compose up -d
```

---

## 最佳实践

1. **使用非 root 用户运行容器**
2. **配置资源限制防止资源耗尽**
3. **启用自动重启策略**
4. **定期备份数据卷**
5. **使用 Ingress TLS 加密**
6. **配置日志轮转**
7. **实施网络策略隔离**
8. **定期进行安全扫描**

---

*最后更新: 2026-05-30*