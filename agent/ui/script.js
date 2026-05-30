/**
 * RDPRemote - 被控端 UI 交互逻辑
 */

// 工具函数
const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => document.querySelectorAll(selector);

// 默认设置
const DEFAULT_SETTINGS = {
    serverUrl: 'ws://localhost:8765',
    autoReconnect: true,
    maxReconnectAttempts: 5,
    reconnectDelay: 3,
    videoQuality: 'medium',
    frameRate: 30,
    adaptiveBitrate: true,
    passwordExpiry: 30,
    requireConfirmation: false,
    logLevel: 'info'
};

// 当前设置（从 localStorage 加载或使用默认值）
let currentSettings = { ...DEFAULT_SETTINGS };

// 加载设置
function loadSettings() {
    try {
        const saved = localStorage.getItem('rdp-remote-settings');
        if (saved) {
            currentSettings = { ...DEFAULT_SETTINGS, ...JSON.parse(saved) };
        }
    } catch (e) {
        console.warn('加载设置失败，使用默认值');
    }
}

// 保存设置
function saveSettings() {
    try {
        localStorage.setItem('rdp-remote-settings', JSON.stringify(currentSettings));
    } catch (e) {
        console.warn('保存设置失败');
    }
}

// 重置设置为默认
function resetSettings() {
    currentSettings = { ...DEFAULT_SETTINGS };
    applySettingsToUI();
    addLog('设置已重置为默认值', 'info');
}

// 将设置应用到 UI
function applySettingsToUI() {
    $('#setting-server-url').value = currentSettings.serverUrl;
    $('#setting-auto-reconnect').checked = currentSettings.autoReconnect;
    $('#setting-max-reconnect-attempts').value = currentSettings.maxReconnectAttempts;
    $('#setting-reconnect-delay').value = currentSettings.reconnectDelay;
    $('#setting-video-quality').value = currentSettings.videoQuality;
    $('#setting-frame-rate').value = currentSettings.frameRate;
    $('#setting-adaptive-bitrate').checked = currentSettings.adaptiveBitrate;
    $('#setting-password-expiry').value = currentSettings.passwordExpiry;
    $('#setting-require-confirmation').checked = currentSettings.requireConfirmation;
    $('#setting-log-level').value = currentSettings.logLevel;
}

// 从 UI 获取设置
function getSettingsFromUI() {
    return {
        serverUrl: $('#setting-server-url').value || 'ws://localhost:8765',
        autoReconnect: $('#setting-auto-reconnect').checked,
        maxReconnectAttempts: parseInt($('#setting-max-reconnect-attempts').value) || 5,
        reconnectDelay: parseInt($('#setting-reconnect-delay').value) || 3,
        videoQuality: $('#setting-video-quality').value,
        frameRate: parseInt($('#setting-frame-rate').value) || 30,
        adaptiveBitrate: $('#setting-adaptive-bitrate').checked,
        passwordExpiry: parseInt($('#setting-password-expiry').value) || 30,
        requireConfirmation: $('#setting-require-confirmation').checked,
        logLevel: $('#setting-log-level').value
    };
}

// 打开设置面板
function openSettingsPanel() {
    $('#settings-panel').classList.remove('hidden');
    $('#settings-overlay').classList.remove('hidden');
    applySettingsToUI();
    addLog('设置面板已打开', 'info');
}

// 关闭设置面板
function closeSettingsPanel() {
    $('#settings-panel').classList.add('hidden');
    $('#settings-overlay').classList.add('hidden');
}

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
    // 加载保存的设置
    loadSettings();
    
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
    
    // 设置按钮
    $('#btn-settings').addEventListener('click', openSettingsPanel);
    
    // 关闭设置面板按钮
    $('#btn-close-settings').addEventListener('click', closeSettingsPanel);
    
    // 遮罩层点击关闭
    $('#settings-overlay').addEventListener('click', closeSettingsPanel);
    
    // 重置设置按钮
    $('#btn-reset-settings').addEventListener('click', () => {
        if (confirm('确定要重置所有设置为默认值吗？')) {
            resetSettings();
        }
    });
    
    // 保存设置按钮
    $('#btn-save-settings').addEventListener('click', () => {
        currentSettings = getSettingsFromUI();
        saveSettings();
        addLog('设置已保存', 'success');
        
        // 根据日志级别过滤日志显示
        applyLogLevel(currentSettings.logLevel);
        
        // 关闭设置面板
        closeSettingsPanel();
    });
    
    // ESC 键关闭设置面板
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && !$('#settings-panel').classList.contains('hidden')) {
            closeSettingsPanel();
        }
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

// 应用日志级别过滤
function applyLogLevel(level) {
    const logLevels = { debug: 0, info: 1, warning: 2, error: 3 };
    const currentLevel = logLevels[level] ?? 1;
    
    const entries = $$('#log-container .log-entry');
    entries.forEach(entry => {
        const entryType = entry.classList.contains('debug') ? 'debug' :
                          entry.classList.contains('info') ? 'info' :
                          entry.classList.contains('warning') ? 'warning' :
                          entry.classList.contains('error') ? 'error' : 'info';
        
        const entryLevel = logLevels[entryType] ?? 1;
        if (entryLevel < currentLevel) {
            entry.style.display = 'none';
        } else {
            entry.style.display = '';
        }
    });
}

// 重写 addLog 以支持日志级别
const originalAddLog = addLog;
addLog = function(message, type = 'info') {
    const logLevels = { debug: 0, info: 1, warning: 2, error: 3 };
    const currentLevel = logLevels[currentSettings.logLevel] ?? 1;
    const messageLevel = logLevels[type] ?? 1;
    
    // 如果当前日志级别低于设置级别，不显示
    if (messageLevel < currentLevel) {
        return;
    }
    
    originalAddLog(message, type);
};
