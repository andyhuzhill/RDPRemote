// RDPRemote Client UI Script

document.addEventListener('DOMContentLoaded', function() {
    const form = document.getElementById('connect-form');
    const deviceCodeInput = document.getElementById('device-code');
    const passwordInput = document.getElementById('password');
    const connectBtn = document.getElementById('connect-btn');
    const statusDiv = document.getElementById('status');

    // Show status message
    function showStatus(message, type) {
        statusDiv.textContent = message;
        statusDiv.className = 'status ' + type;
    }

    // Hide status message
    function hideStatus() {
        statusDiv.className = 'status';
    }

    // Set loading state
    function setLoading(isLoading) {
        connectBtn.disabled = isLoading;
        if (isLoading) {
            connectBtn.innerHTML = '<span class="loading"></span>连接中...';
        } else {
            connectBtn.textContent = '连接';
        }
    }

    // Form submission handler
    form.addEventListener('submit', async function(e) {
        e.preventDefault();
        
        const deviceCode = deviceCodeInput.value.trim();
        const password = passwordInput.value;

        // Validate inputs
        if (!deviceCode) {
            showStatus('请输入设备代码', 'error');
            deviceCodeInput.focus();
            return;
        }

        if (!password) {
            showStatus('请输入密码', 'error');
            passwordInput.focus();
            return;
        }

        // Disable button and show loading
        setLoading(true);
        hideStatus();

        try {
            // TODO: Replace with actual WebRTC connection logic
            // This is a placeholder for the connection flow
            console.log('Connecting to device:', deviceCode);
            
            // Simulate connection attempt (replace with real API call)
            // const response = await fetch('/api/connect', {
            //     method: 'POST',
            //     headers: { 'Content-Type': 'application/json' },
            //     body: JSON.stringify({ deviceCode, password })
            // });
            
            // For now, just show a success message
            await new Promise(resolve => setTimeout(resolve, 1000));
            
            showStatus('正在建立连接...', 'info');
            
            // Initialize WebRTC peer connection
            // This will be implemented in the Rust client
            // The JavaScript will communicate via WebSocket with the Rust backend
            
        } catch (error) {
            console.error('Connection error:', error);
            showStatus('连接失败: ' + error.message, 'error');
        } finally {
            setLoading(false);
        }
    });

    // Allow Enter key to submit
    passwordInput.addEventListener('keydown', function(e) {
        if (e.key === 'Enter') {
            form.dispatchEvent(new Event('submit'));
        }
    });

    // Clear status on input
    [deviceCodeInput, passwordInput].forEach(input => {
        input.addEventListener('input', hideStatus);
    });
});
