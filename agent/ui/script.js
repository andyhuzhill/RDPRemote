/**
 * RDPRemote - 被控端 UI 交互逻辑
 */

// 工具函数
const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => document.querySelectorAll(selector);

// 生成随机设备代码
function generateDeviceCode() {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    let code = 'RDPR-';
    for (let i = 0; i < 8; i++) {
        code += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return code;
}

// 生成随机连接密码
function generatePassword() {
    const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    let password = '';
    for (let i = 0; i < 6; i++) {
        password += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return password;
}

// 获取平台信息
function getPlatformInfo() {
    const ua = navigator.userAgent;
    if (ua.includes('Windows')) return 'Windows';
    if (ua.includes('Mac')) return 'macOS';
    if (ua.includes('Linux')) return 'Linux';
    if (ua.includes('Android')) return 'Android';
    if (ua.includes('iPhone') || ua.includes('iPad')) return 'iOS';
    return 'Unknown';
}

// 添加日志
function addLog(message, type = 'info') {
    const container = $('#log-container');
    const entry = document.createElement('p');
    entry.className = `log-entry ${type}`;
    
    const timestamp = new Date().toLocaleTimeString('zh-CN');
    entry.textContent = `[${timestamp}] ${message}`;
    
    container.appendChild(entry);
    container.scrollTop = container.scrollHeight;
    
    // 保留最近 50 条日志
    while (container.children.length > 50) {
        container.removeChild(container.firstChild);
    }
}

// 更新状态显示
function updateStatus(status) {
    const statusEl = $('#status');
    statusEl.textContent = status;
    statusEl.className = 'value status';
    
    switch (status) {
        case '等待连接':
            statusEl.classList.add('waiting');
            break;
        case '已连接':
            statusEl.classList.add('connected');
            break;
        case '已断开':
            statusEl.classList.add('disconnected');
            break;
    }
}

// 初始化
function init() {
    // 生成设备代码
    const deviceCode = generateDeviceCode();
    $('#device-code').textContent = deviceCode;
    
    // 显示平台信息
    $('#platform-info').textContent = getPlatformInfo();
    
    // 生成初始密码
    const password = generatePassword();
    $('#connection-password').textContent = password;
    
    // 初始状态
    updateStatus('等待连接');
    addLog('被控端已启动', 'info');
    addLog(`设备代码: ${deviceCode}`, 'info');
    addLog('等待控制端连接...', 'warning');
    
    // 复制密码按钮
    $('#copy-password').addEventListener('click', async () => {
        const password = $('#connection-password').textContent;
        try {
            await navigator.clipboard.writeText(password);
            addLog('密码已复制到剪贴板', 'success');
            
            const btn = $('#copy-password');
            const originalText = btn.textContent;
            btn.textContent = '已复制!';
            setTimeout(() => {
                btn.textContent = originalText;
            }, 1500);
        } catch (err) {
            addLog('复制失败，请手动复制', 'error');
        }
    });
    
    // 重新生成密码按钮
    $('#btn-regenerate').addEventListener('click', () => {
        const newPassword = generatePassword();
        $('#connection-password').textContent = newPassword;
        addLog('密码已重新生成', 'warning');
    });
    
    // 刷新设备信息按钮
    $('#btn-refresh').addEventListener('click', () => {
        $('#device-code').textContent = generateDeviceCode();
        $('#platform-info').textContent = getPlatformInfo();
        addLog('设备信息已刷新', 'info');
    });
    
    // 模拟连接状态变化（实际使用时由后端 WebSocket 控制）
    window.updateConnectionStatus = (status) => {
        updateStatus(status);
        if (status === '已连接') {
            addLog('控制端已连接', 'success');
        } else if (status === '已断开') {
            addLog('连接已断开', 'warning');
        }
    };
    
    window.updatePassword = (password) => {
        $('#connection-password').textContent = password;
        addLog('连接密码已更新', 'info');
    };
}

// 页面加载完成后初始化
document.addEventListener('DOMContentLoaded', init);
